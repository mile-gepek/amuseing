#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use amuseing::playback::Song;

slint::include_modules!();

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    // let songs: Vec<Song> = (5..25).map(|i| {
    //     Song::new(i, format!("Song {i}"), i as u16)
    // }).collect();
    // let songs_model: slint::VecModel<SongModel> = songs.iter().map(|song| song.clone().into()).collect();
    // ui.set_songs(Rc::new(songs_model).into());

    // let t: Vec<SongModel> = ui.get_songs().iter().collect();
    // println!("{:?}", t);

    ui.run()?;

    Ok(()) 
}
