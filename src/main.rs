#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use amuseing::playback::{Player, Playlist, Song, Volume};
use amuseing::queue::{Queue, RepeatMode};
use slint::{ModelRc, VecModel};

slint::include_modules!();

impl From<Song> for SongModel {
    fn from(song: Song) -> Self {
        Self {
            id: *song.id() as i32,
            duration: song.duration().as_secs() as i32,
            title: song.title().into(),
        }
    }
}

use std::{cell::RefCell, error::Error, rc::Rc};

fn main() -> Result<(), Box<dyn Error>> {
    let app = AppWindow::new()?;

    let playlist = Playlist::new(
        "C:\\Users\\leola\\Music".into(),
        "BLAAA".to_string(),
        "BLAAA".into(),
    )?;
    let mut queue = Queue::new(RepeatMode::All);
    let songs = playlist.songs()?;
    queue.extend(songs.clone());
    let songs_as_model: Vec<SongModel> = songs.into_iter().map(|song| song.into()).collect();
    app.set_songs(ModelRc::new(VecModel::from_slice(&songs_as_model)));
    let volume = Volume::from_percent(0.5);
    let mut player = Player::with_queue(queue, volume);
    player.run().unwrap();
    let player = Rc::new(RefCell::new(player));

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_pause_play(move || {
        let player = player_copy.borrow();
        if player.is_paused() {
            player.resume();
        } else {
            player.pause();
        }
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_start_song(move |playlist_id, song_id| {
            let mut player = player_copy.borrow_mut();
            {
                let mut queue = player.queue_mut();
                let _ = queue.jump(song_id as usize);
            }
            let _ = player.run();
        });

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_is_paused(move || {
        let player = player_copy.borrow();
        player.is_paused()
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_toggle_repeat_mode(move || {
            let mut player = player_copy.borrow_mut();
            let mut queue = player.queue_mut();
            queue.repeat_mode = queue.repeat_mode.next();
        });

    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_change_volume(move |percent| {
            let mut player = player_copy.borrow_mut();
            player.set_volume(Volume::from_percent(percent as f64));
        });

    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_get_volume(move || -> f32 {
            let player = player_copy.borrow_mut();
            *player.volume().percent() as f32
        });

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_fast_forward(move || {
        let mut player = player_copy.borrow_mut();
        player.fast_forward();
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_rewind(move || {
        let mut player = player_copy.borrow_mut();
        player.rewind();
    });

    app.run()?;

    Ok(())
}
