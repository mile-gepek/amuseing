use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::{fs, path::PathBuf, time::Duration};
use symphonia::core::audio::{AudioBuffer, Signal};
use symphonia::core::units;
use symphonia::core::{
    codecs::Decoder,
    errors::Result as SymphoniaResult,
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
};
use symphonia_bundle_mp3::{MpaDecoder, MpaReader};

type SampleType = f64;

slint::include_modules!();
use slint::SharedString;

use crate::errors::{OutOfBoundsError, PlayerRunningError, SeekError};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RepeatMode {
    Off,
    Single,
    All,
}

/// Represents a song from a `SongQueue`.
///
/// Songs are played from a `Player`, which uses a Symphonia reader and decoder from `Song::reader_decoder` to read the samples from the file.
///
/// Songs should be created with `Song::from_path`.
///
/// The duration of the song is automatically calculated when created.
#[derive(Clone, Debug)]
pub struct Song {
    id: u32,
    title: String,
    path: PathBuf,
    duration: Duration,
}

impl Song {
    fn new(id: u32, title: String, path: PathBuf, duration: Duration) -> Self {
        Self {
            id,
            title,
            path,
            duration,
        }
    }

    /// Create a new Song from a mp3 file at `path`, and automatically calculate the duration from it.
    pub fn from_path(title: String, path: PathBuf) -> SymphoniaResult<Song> {
        let reader = Self::reader(&path)?;
        let track = reader
            .default_track()
            .expect("Found mp3 file without a track, abort");
        let params = &track.codec_params;
        let time_base = params
            .time_base
            .expect("Every mp3 track should have a time base");
        let n_frames = params.n_frames.expect("Every mp3 track should have frames");
        let duration = time_base.calc_time(n_frames).into();
        Ok(Self::new(track.id, title, path, duration))
    }

    // Feels kinda dumb to have to get a reader for duration, and later for actually reading the data
    fn reader(path: &PathBuf) -> SymphoniaResult<MpaReader> {
        let file = fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut reader_options = FormatOptions::default();
        reader_options.enable_gapless = true;
        MpaReader::try_new(mss, &reader_options)
    }

    /// Try to get a reader and decoder for use in `Player` to get audio samples
    fn reader_decoder(&self) -> SymphoniaResult<(MpaReader, MpaDecoder)> {
        let reader = Self::reader(&self.path)?;
        let track = reader
            .default_track()
            .expect("Every mp3 file should have a track");
        let decoder = MpaDecoder::try_new(&track.codec_params, &Default::default())?;
        Ok((reader, decoder))
    }
}

impl Into<SongModel> for Song {
    fn into(self) -> SongModel {
        SongModel {
            id: self.id as i32,
            duration: self.duration.as_secs() as i32,
            title: SharedString::from(&self.title),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SongQueue {
    pub songs: Vec<Song>,
    index: usize,
    pub repeat_mode: RepeatMode,
    /// Used for proper iteration after skipping/jumping, and initial `next` call
    has_advanced: bool,
}

impl SongQueue {
    pub fn new(repeat_mode: RepeatMode) -> Self {
        Self {
            songs: Vec::new(),
            index: 0,
            repeat_mode,
            has_advanced: false,
        }
    }

    pub fn next(&mut self) -> Option<&Song> {
        if self.songs.is_empty() {
            return None;
        }
        if self.repeat_mode != RepeatMode::Single && self.has_advanced == true {
            if self.index < self.songs.len() - 1 {
                self.index += 1;
            } else if self.repeat_mode == RepeatMode::All {
                self.index = (self.index + 1) % self.songs.len();
            }
        }
        self.has_advanced = true;
        self.songs.get(self.index)
    }

    pub fn current(&self) -> Option<Song> {
        self.songs.get(self.index).cloned()
    }

    /// Jump to index `n` in the queue
    ///
    /// Ensures the next item is at index `n`
    pub fn jump(&mut self, new_index: usize) -> Result<(), OutOfBoundsError<usize>> {
        if new_index > self.songs.len() {
            return Err(OutOfBoundsError::High {
                value: new_index,
                max: self.songs.len(),
            });
        }
        self.has_advanced = false;
        self.index = new_index;
        Ok(())
    }

    /// Skip n items forward, ensures the next item is `n` ahead;
    ///
    /// If the repeat mode is Off, skipping beyond the end of the queue will set the index to the length of the queue, otherwise wrap around to the beginning
    pub fn skip(&mut self, n: usize) {
        let new_index = if !self.songs.is_empty()
            || (self.songs.len() - self.index) > n && self.repeat_mode == RepeatMode::Off
        {
            self.songs.len()
        } else {
            (self.index + n) % self.songs.len()
        };
        self.jump(new_index)
            .expect("Calculated jump from skip shouldn't fail");
    }

    /// Rewind n items, ensures the next item is `n` behind the current item
    pub fn rewind(&mut self, n: usize) {
        let new_index = if self.songs.is_empty() {
            0
        } else {
            if n <= self.index {
                self.index - n
            } else {
                self.songs.len() - (self.index - n)
            }
        };
        self.jump(new_index)
            .expect("Calculated jump from rewind shouldn't fail");
    }
}

// Do I make songs pub or a getter or deref??????????

// impl Deref for SongQueue {
//     type Target = Vec<Song>;

//     fn deref(&self) -> &Self::Target {
//         &self.songs
//     }
// }

// impl DerefMut for SongQueue {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.songs
//     }
// }

#[derive(Debug)]
pub enum PlayerState {
    Paused,
    Playing,
    Finished,
    NotStarted,
}

pub enum PlayerMessage {
    Stop,
    Pause,
    Resume,
    Seek(Duration),
    Quit,
}

#[derive(Debug)]
pub struct Volume(pub f64);

impl Volume {
    /// Calculates the sample multiplier depending on a percentage to adjust for human hearing.
    ///
    /// The curve is: `(e^(B*percent) - 1) / e^B`, where B is 7.0.
    pub fn from_percentage(percent: f64) -> Self {
        const B: f64 = 7.0;
        let volume = ((B * percent).exp() - 1.) / B.exp();
        Self(volume)
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    queue: Arc<Mutex<SongQueue>>,
    state: Arc<RwLock<PlayerState>>,
    /// None if the player hasn't started yes, the player's state is `PlayerState::NotStarted` in this case
    sender: Option<Sender<PlayerMessage>>,
    time_playing: Arc<RwLock<Duration>>,
    volume: Arc<RwLock<Volume>>,
}

impl Player {
    pub fn new(volume: Volume) -> Self {
        Self {
            queue: Arc::new(Mutex::new(SongQueue::new(RepeatMode::Off))),
            state: Arc::new(RwLock::new(PlayerState::NotStarted)),
            sender: None,
            time_playing: Arc::new(RwLock::new(Duration::from_secs(0))),
            volume: Arc::new(RwLock::new(volume)),
        }
    }

    /// If available, return a cloned version of the Song that's currently playing.
    pub fn current(&self) -> Option<Song> {
        let queue_lock = self.queue.lock().unwrap();
        queue_lock.current()
    }

    /// Send a message to the audio playing thread.
    ///
    /// Returns true on successful messages.
    pub fn send_message(&self, message: PlayerMessage) -> bool {
        let Some(tx) = &self.sender else { return false };
        tx.send(message).is_ok()
    }

    /// Send a message to the audio thread to quit playing entirely.
    /// 
    /// NOTE: the player's state is NOT updated to Finished by this call, but by the audio thread
    pub fn quit(&self) -> bool {
        self.send_message(PlayerMessage::Quit)
    }

    /// Send a message to the audio thread to stop the current song.
    ///
    /// If there are more songs available, they will be played.
    pub fn stop(&self) -> bool {
        self.send_message(PlayerMessage::Stop)
    }

    /// Send a message to the audio thread to pause playback.
    /// 
    /// NOTE: the player's state is NOT updated to Paused by this call, but by the audio thread
    pub fn pause(&self) -> bool {
        self.send_message(PlayerMessage::Pause)
    }

    /// Send a message to the audio thread to resume playback.
    /// 
    /// NOTE: the player's state is NOT updated to Playing by this call, but by the audio thread
    pub fn resume(&self) -> bool {
        self.send_message(PlayerMessage::Resume)
    }

    /// Start the player.
    pub fn run(&mut self) -> Result<(), PlayerRunningError> {
        {
            let mut state_lock = self.state.write().unwrap();
            match *state_lock {
                PlayerState::Playing | PlayerState::Paused => return Err(PlayerRunningError),
                _ => {}
            }
            *state_lock = PlayerState::Paused;
        }
        let queue = self.queue.clone();
        let player_state = self.state.clone();
        let time_playing = self.time_playing.clone();
        let volume = self.volume.clone();

        let (tx, rx) = mpsc::channel::<PlayerMessage>();
        self.sender = Some(tx);
        let (device, stream_config) = init_cpal();
        let stream_channels = stream_config.channels() as usize;
        let (mut producer, consumer) = {
            let buf: HeapRb<f64> = HeapRb::new(32 * 1024);
            buf.split()
        };
        let audio_stream = match stream_config.sample_format() {
            SampleFormat::I8 => {
                create_stream::<u8>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::I16 => {
                create_stream::<i16>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::I32 => {
                create_stream::<i32>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::I64 => {
                create_stream::<i64>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::U8 => {
                create_stream::<u8>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::U16 => {
                create_stream::<u16>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::U32 => {
                create_stream::<u32>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::U64 => {
                create_stream::<u64>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::F32 => {
                create_stream::<f32>(device, &stream_config.into(), consumer, volume)
            }
            SampleFormat::F64 => {
                create_stream::<f64>(device, &stream_config.into(), consumer, volume)
            }
            sample_format => panic!("Unsupported sample format: '{sample_format}'"),
        }
        .unwrap();
        audio_stream.play().unwrap();
        thread::spawn(move || {
            'main_loop: loop {
                let song = {
                    let mut queue_lock = queue.lock().unwrap();
                    let Some(song) = queue_lock.next() else {
                        break;
                    };
                    song.clone()
                };
                let (mut reader, mut decoder) = song.reader_decoder().unwrap();
                let track = reader.default_track().unwrap();
                let channel_factor = stream_channels / track.codec_params.channels.unwrap().count();
                let track_id = track.id;
                let time_base = track.codec_params.time_base.unwrap();
                {
                    let mut time_playing_lock = time_playing.write().unwrap();
                    *time_playing_lock = Default::default();

                    let mut state_lock = player_state.write().unwrap();
                    *state_lock = PlayerState::Playing;
                }

                let mut playing = true;
                let mut source_exhausted = false;
                let mut sample_deque: VecDeque<SampleType> = VecDeque::new();
                while !source_exhausted || !producer.is_empty() {
                    match rx.recv() {
                        Ok(message) => match message {
                            PlayerMessage::Quit => break 'main_loop,
                            PlayerMessage::Stop => break,
                            PlayerMessage::Pause => {
                                let mut state_lock = player_state.write().unwrap();
                                *state_lock = PlayerState::Paused;
                                playing = false;
                                // We can slow down the thread a bit if the player is paused
                                thread::sleep(Duration::from_millis(100));
                            }
                            PlayerMessage::Resume => {
                                let mut state_lock = player_state.write().unwrap();
                                *state_lock = PlayerState::Playing;
                                playing = true;
                            }
                            PlayerMessage::Seek(dur) => {
                                use symphonia::core::formats::{SeekMode, SeekTo};
                                let time: units::Time = dur.into();
                                // FormatReader is seekable depending on the MediaSourceStream.is_seekable() method
                                // I'm fairly certain this should always be true for mp3 files
                                // TODO: The bool `seekable` should be used to check if we can seek, I don't know how to handle that yet
                                let seeked_to = reader
                                    .seek(
                                        SeekMode::Coarse,
                                        SeekTo::Time {
                                            time,
                                            track_id: Some(track_id),
                                        },
                                    )
                                    .expect("Mp3 readers should always be seekable");
                                let mut time_playing_lock = time_playing.write().unwrap();
                                let time = time_base.calc_time(seeked_to.actual_ts);
                                *time_playing_lock = time.into();
                                // Reset the decoder after seeking, the docs say this is a necessary step after seeking
                                decoder.reset();
                            }
                        },
                        // Break if the channel is disconnected, because this means the sender was dropped
                        Err(mpsc::RecvError) => break,
                    }
                    if !playing {
                        continue;
                    }
                    if !sample_deque.is_empty() {
                        // If there are samples available, write them to the producer if there is space
                        while producer.vacant_len() >= channel_factor {
                            let Some(sample) = sample_deque.pop_front() else {
                                break;
                            };
                            for _ in 0..channel_factor {
                                producer.try_push(sample).unwrap();
                            }
                        }
                    } else {
                        // Push samples for the sample deque if none are available

                        // TODO: figure out resampling

                        if let Ok(packet) = reader.next_packet() {
                            {
                                let mut duration_lock = time_playing.write().unwrap();
                                *duration_lock = time_base.calc_time(packet.ts()).into();
                            }
                            source_exhausted = false;
                            let audio_buf = decoder.decode(&packet).unwrap();
                            let mut audio_buf_type: AudioBuffer<SampleType> =
                                audio_buf.make_equivalent();
                            audio_buf.convert(&mut audio_buf_type);
                            for (l, r) in audio_buf_type
                                .chan(0)
                                .iter()
                                .zip(audio_buf_type.chan(1).iter())
                            {
                                // In theory, this could make the sample deque grow as large as the song, which probably isn't good
                                sample_deque.push_back(*l);
                                sample_deque.push_back(*r);
                            }
                        } else {
                            source_exhausted = true;
                        }
                    }
                }
            }
            // Set the player's state to Finished after we break out of the loop
            let mut state_lock = player_state.write().unwrap();
            *state_lock = PlayerState::Finished;

            // Set the player's time_playing duration to 0 when finished
            let mut time_playing_lock = time_playing.write().unwrap();
            *time_playing_lock = Default::default();
        });
        Ok(())
    }

    /// Seek to a specific duration of the song, if one is currently playing.
    ///
    /// If the duration is longer than the maximum duration returns an error.
    pub fn seek_duration(&self, duration: Duration) -> Result<bool, SeekError> {
        let duration_max = self.current().ok_or(SeekError::NoCurrentSong)?.duration;
        if duration > duration_max {
            return Err(SeekError::out_of_range(duration, duration_max));
        }
        Ok(self.send_message(PlayerMessage::Seek(duration)))
    }

    /// Skip to the next song.
    pub fn fast_forward(&self) {
        let mut queue_lock = self.queue.lock().unwrap();
        queue_lock.skip(1);
        self.stop();
    }

    /// Rewind to the beginning of the track if it has been playing long enough, otherwise the previous track.
    pub fn rewind(&self) {
        let mut queue_lock = self.queue.lock().unwrap();
        let time_playing = self.time_playing.read().unwrap();
        /// If the current song has been playing for longer than this constant, go back to the beginning of it
        const REWIND_TOLERANCE: f32 = 3.0;
        if time_playing.as_secs_f32() < REWIND_TOLERANCE && self.current().is_some() {
            // Seeking to 0 should never fail, and checking if current is Some guarantees we have a song playing
            let _ = self.seek_duration(Duration::from_secs(0));
        } else {
            queue_lock.rewind(1);
            self.stop();
        }
    }
}

fn init_cpal() -> (cpal::Device, cpal::SupportedStreamConfig) {
    let device = cpal::default_host()
        .default_output_device()
        .expect("no output device available");

    // Create an output stream for the audio so we can play it
    // NOTE: If system doesn't support the file's sample rate, the program will panic when we try to play,
    // so we'll need to resample the audio to a supported config
    let supported_config_range = device
        .supported_output_configs()
        .expect("error querying audio output configs")
        .next()
        .expect("no supported audio config found");

    // Pick the best (highest) sample rate
    (device, supported_config_range.with_max_sample_rate())
}

fn write_audio<T: Sample>(
    data: &mut [T],
    samples: &mut impl Consumer<Item = SampleType>,
    volume: &RwLock<Volume>,
    _cbinfo: &cpal::OutputCallbackInfo,
) where
    T: cpal::FromSample<SampleType>,
{
    // Channel remapping might be done here, to lower the load on the Player thread
    let volume = volume.read().unwrap();
    for d in data.iter_mut() {
        match samples.try_pop() {
            Some(sample) => *d = T::from_sample(sample * volume.0),
            None => *d = T::from_sample(SampleType::EQUILIBRIUM),
        }
    }
}

/// Create a stream to the `device`, reading data from the `consumer`
fn create_stream<T>(
    device: cpal::Device,
    stream_config: &cpal::StreamConfig,
    mut consumer: (impl Consumer<Item = SampleType> + std::marker::Send + 'static),
    volume: Arc<RwLock<Volume>>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + cpal::FromSample<SampleType>,
{
    let callback = move |data: &mut [T], cbinfo: &cpal::OutputCallbackInfo| {
        write_audio(data, &mut consumer, &volume, cbinfo)
    };
    // TODO: set up mpsc between Player thread and cpal thread to get new audio device on disconnect, or otherwise catch other errors
    let err_fn = |e| eprintln!("Stream error: {e}");
    device.build_output_stream(stream_config, callback, err_fn, None)
}
