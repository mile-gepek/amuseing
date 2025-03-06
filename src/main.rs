// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use amuseing::playback::{AtomicVolume, Player, Playlist, Song};
use amuseing::queue::{Queue, RepeatMode};
use slint::{ModelRc, ToSharedString, VecModel};

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
        // "C:\\Users\\leola\\Music".into(),
        "/home/leo/music".into(),
        "BLAAA".to_string(),
        "BLAAA".into(),
    )?;
    let mut queue = Queue::new(RepeatMode::All);
    let songs = playlist.songs()?;
    queue.extend(songs.clone());
    let songs_as_model: Vec<SongModel> = songs.into_iter().map(|song| song.into()).collect();
    app.set_songs(ModelRc::new(VecModel::from_slice(&songs_as_model)));
    let volume = 0.5;
    let mut player = Player::with_queue(queue, volume);

    player.run().unwrap();
    let player = Rc::new(RefCell::new(player));

    let app_weak = app.as_weak();
    let player_copy = player.clone();
    app.global::<PlayerControls>().on_pause_play(move || {
        let player = player_copy.borrow();
        let app = app_weak.unwrap();
        let pause_button_text = if player.is_paused() {
            player.resume();
            "Playing"
        } else {
            player.pause();
            "Paused"
        };
        let global = app.global::<PlayerControls>();
        global.set_pause_button_text(pause_button_text.into());
    });

    let app_weak = app.as_weak();
    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_start_song(move |_playlist_id, song_id| {
            let mut player = player_copy.borrow_mut();
            let app = app_weak.unwrap();
            {
                let mut queue = player.queue_mut();
                let _ = queue.jump(song_id as usize);
            }
            let _ = player.run();
            let global = app.global::<PlayerControls>();
            global.set_is_running(true);
        });

    let app_weak = app.as_weak();
    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_toggle_repeat_mode(move || {
            let app = app_weak.unwrap();
            let mut player = player_copy.borrow_mut();
            let mut queue = player.queue_mut();
            queue.repeat_mode = queue.repeat_mode.next();
            app.global::<PlayerControls>()
                .set_repeat_mode_text(queue.repeat_mode.to_shared_string());
        });

    let player_copy = player.clone();
    let app_copy = app.as_weak();
    app.global::<PlayerControls>()
        .on_change_volume(move |percent| {
            let mut player = player_copy.borrow_mut();
            player.set_volume(&AtomicVolume::from_percent(percent as f64));
            let app = app_copy.unwrap();
            app.global::<PlayerControls>().set_volume(percent);
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

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_seek(move |percent| {
        let mut player = player_copy.borrow_mut();
        let Some(current) = player.current() else {
            return;
        };
        let duration = current.duration();
        let duration_seek = duration.mul_f32(percent);
        let _ = player.seek_duration(duration_seek);
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>()
        .on_time_playing_percent(move || {
            let player = player_copy.borrow();
            let Some(current) = player.current() else {
                return 0.;
            };
            let duration = current.duration().as_secs_f64();
            let time_playing = player.time_playing().as_secs_f64();
            (time_playing / duration) as f32
        });

    // let player_copy = player.clone();
    // app.global::<PlayerControls>().on_is_running(move || {
    //     let player = player_copy.borrow();
    //     player.current().is_some()
    // });

    app.run()?;

    Ok(())
}
