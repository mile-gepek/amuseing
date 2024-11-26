#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use amuseing::playback::{Player, Playlist, Song, SongQueue, Volume};

slint::include_modules!();

use std::{cell::RefCell, error::Error, rc::Rc};

fn main() -> Result<(), Box<dyn Error>> {
    let app = AppWindow::new()?;

    let playlist = Playlist::new("C:\\Users\\leola\\Music".into(), "BLAAA".to_string(), "BLAAA".into())?;
    let mut queue = SongQueue::new(amuseing::playback::RepeatMode::All);
    queue.songs = playlist.songs()?;
    let mut player = Player::with_queue(queue, Volume::from_percentage(0.5));
    let _ = player.run();
    let player = Rc::new(RefCell::new(player));

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_pause_play(move || {
        let player = player_copy.borrow();
        let is_paused = player.is_paused();
        if is_paused {
            player.resume();
        } else {
            player.pause();
        }
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_start_song(move |playlist_id, song_id| {
        let player = player_copy.borrow();
        let mut queue = player.queue_mut();
        let _ = queue.jump(song_id as usize);
    });

    let player_copy = player.clone();
    app.global::<PlayerControls>().on_is_paused(move || {
        let player = player_copy.borrow_mut();        
        player.is_paused()
    });

    app.run()?;

    Ok(()) 
}
