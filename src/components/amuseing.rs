use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Receiver,
    time::Duration,
};

use amuseing::{
    config::Config,
    playback::{Player, PlayerUpdate, Playlist},
};
use dioxus::{logger::tracing, prelude::*};

#[derive(Copy, Clone, Debug)]
struct PlayerInfo {
    player: Signal<Player>,
    player_update: Signal<Option<Receiver<PlayerUpdate>>>,
    selected_playlist_index: Signal<Option<usize>>,
    active_indexes: Signal<Option<(usize, usize)>>,
    is_paused: Signal<bool>,
    seek_bar_position: Signal<f64>,
}

impl PlayerInfo {
    fn new(player: Player, player_update: Option<Receiver<PlayerUpdate>>) -> Self {
        Self {
            player: Signal::new(player),
            player_update: Signal::new(player_update),
            selected_playlist_index: Signal::new(None),
            active_indexes: Signal::new(None),
            is_paused: Signal::new(false),
            seek_bar_position: Signal::new(0.),
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct UpdateSeekBar(Signal<bool>);

impl Deref for UpdateSeekBar {
    type Target = Signal<bool>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UpdateSeekBar {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Copy, Clone, Debug)]
struct PlaylistsState {
    playlists: Signal<Vec<Signal<Playlist>>>,
}
impl PlaylistsState {
    fn new(playlists: &[Playlist]) -> Self {
        Self {
            playlists: Signal::new(
                playlists
                    .to_vec()
                    .into_iter()
                    .map(|playlist| Signal::new(playlist))
                    .collect(),
            ),
        }
    }
}

#[derive(PartialEq, Props, Clone)]
struct PlaylistProp {
    playlist: Playlist,
    index: usize,
    selected: bool,
}

#[component]
fn PlaylistButton(props: PlaylistProp) -> Element {
    let mut div_class = "playlist-button".to_string();
    let mut is_valid = use_signal(|| props.playlist.is_valid());
    if !is_valid() {
        div_class += " playlist-invalid";
    } else if props.selected {
        div_class += " playlist-selected";
    }
    static INVALID_PLAYLIST_ICON: Asset = asset!("/assets/icons/warning.svg");
    let mut selected_index = use_context::<PlayerInfo>().selected_playlist_index;
    let index = props.index;
    rsx! {
        button {
            class: div_class,
            onclick: move |_| {
                selected_index.set(Some(index));
                is_valid.set(props.playlist.is_valid());
            },
            p {
                { props.playlist.name() }
            }
            if !is_valid() {
                img {
                    class: "invalid-playlist-icon",
                    src: INVALID_PLAYLIST_ICON,
                }
            },
        }
    }
}

#[component]
fn PlaylistPanel() -> Element {
    let playlists = use_context::<PlaylistsState>().playlists;
    let selected_index = use_context::<PlayerInfo>().selected_playlist_index;
    rsx! {
        div {
            class: "playlist-panel",
            div {
                class: "playlist-panel-buttons",
                button {
                    "+"
                }
                button {
                    "0"
                }
                button {

                }
            }
            div {
                class: "playlist-list",
                for (i, playlist) in playlists.read().iter().enumerate() {
                    PlaylistButton {
                        playlist: playlist.read().clone(),
                        index: i,
                        selected: selected_index.read().is_some_and(|index| index == i),
                    }
                }
            }
        }
    }
}

#[component]
fn SeekBar() -> Element {
    let mut player_info = use_context::<PlayerInfo>();
    let mut player = player_info.player;
    let mut should_update = use_context::<UpdateSeekBar>();

    let seek = move |event: Event<FormData>| {
        let value = event.value().parse::<f64>().unwrap();
        player_info.seek_bar_position.set(value);
        let percent = value / 100.;
        let Some(song) = player.read().current() else {
            return;
        };
        let duration = song.duration();
        let _ = player.write().seek_duration(duration.mul_f64(percent));
    };

    let value = player_info.seek_bar_position;

    rsx! {
        div {
            id: "seek-bar",
            input {
                r#type: "range",
                min: 0.,
                max: 100.,
                step: 0.1,
                value: value,
                onchange: seek,
                oninput: move |_| should_update.set(false),
                onclick: move |_| should_update.set(true)
            }
        }
    }
}

fn format_time(mut seconds: u64, show_hours: bool) -> String {
    let mut formatted = String::new();
    let mut minutes = seconds / 60;
    seconds %= 60;
    if show_hours {
        let hours = minutes / 60;
        minutes %= 60;
        formatted += format!("{:02}:", hours).as_str();
    }
    formatted += format!("{:02}:", minutes).as_str();
    formatted += format!("{:02}", seconds).as_str();
    formatted
}

#[component]
fn SongDisplay() -> Element {
    let player = use_context::<PlayerInfo>().player;
    let player_read = player.read();
    if let Some(song) = player_read.current() {
        let title = song.title();
        let duration = song.duration().as_secs();
        let time_playing = player_read.time_playing().as_secs();
        let show_hours = duration > 3600;
        rsx! {
            div {
                class: "song-display",
                p {{ title }}
                p {
                    { format_time(time_playing, show_hours) }
                    " / "
                    { format_time(duration, show_hours) }
                }
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
fn CenterControls() -> Element {
    let player_info = use_context::<PlayerInfo>();
    let mut player = player_info.player;
    let mut is_paused = player_info.is_paused;
    rsx! {
        div {
            class: "controls-center",
            button {
                onclick: move |_| {
                    player.write().rewind();
                },
                "Re",
            }
            button {
                onclick: move |_| {
                    if is_paused() {
                        player.write().resume();
                        is_paused.set(false);
                    } else {
                        player.write().pause();
                        is_paused.set(true);
                    }
                },
                if is_paused() { "Resume" } else { "Pause" }
            }
            button {
                onclick: move |_| {
                    player.write().fast_forward();
                    player.write().resume();
                },
                "FF"
            }
        }
    }
}

#[component]
fn RightControls() -> Element {
    rsx! {
        div {
            class: "controls-right",
            p {
                "AMOGUS"
            }
        }
    }
}

#[component]
fn BottomPanel() -> Element {
    rsx! {
        div {
            class: "content-wrapper",

            PlaylistPanel { }
        }

        footer {
            class: "bottom-panel",

            SeekBar {  }

            span {
                class: "controls",

                SongDisplay {  }

                CenterControls {  }

                RightControls {  }
            }
        }
    }
}

#[component]
pub fn Amuseing() -> Element {
    let config = Config::from_default_path().unwrap_or_default();
    let playlists = config.playlists.clone();
    let mut player = Player::new(config.player.volume);
    let songs = config.playlists[1].songs().unwrap();
    player.set_songs(songs);
    let player_update = player.run(config.player.buffer_size).ok();
    let mut player_info = use_context_provider(|| PlayerInfo::new(player, player_update));
    let config_context = use_context_provider(|| Signal::new(config));
    let playlists_context = use_context_provider(|| PlaylistsState::new(playlists.inner()));
    let update_seek_bar = use_context_provider(|| UpdateSeekBar(Signal::new(true)));

    // 100ms loop to update any component that depends on `player`
    spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            let is_paused = player_info.player.read().is_paused();
            player_info.is_paused.set(is_paused);
            // Dummy write to update all components that depend on player
            player_info.player.write();

            if *update_seek_bar.read() {
                let percent = if let Some(song) = player_info.player.read().current() {
                    let duration = song.duration().as_secs_f64();
                    let time_playing = player_info.player.read().time_playing().as_secs_f64();
                    time_playing / duration
                } else {
                    0.
                };
                player_info.seek_bar_position.set(percent * 100.);
            }
            if let Some(player_update) = player_info.player_update.as_mut() {
                for message in player_update.try_iter() {
                    match message {
                        PlayerUpdate::SongChange { song_info } => {
                            if let Some((new_song_index, _)) = song_info {
                                if let Some((_, active_song_index)) =
                                    player_info.active_indexes.write().as_mut()
                                {
                                    *active_song_index = new_song_index;
                                }
                            } else {
                                player_info.active_indexes.set(None);
                            }
                        }
                        message => {
                            tracing::debug!("{:?}", message);
                        }
                    }
                }
            }
        }
    });

    rsx! {
        BottomPanel {  }
    }
}
