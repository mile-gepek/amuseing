use clap::Parser;
use log::{debug, error, info, warn};

use std::{sync::mpsc::Receiver, time::Duration};

use amuseing::{
    config::Config,
    errors::PlayerStartError,
    playback::{Player, PlayerUpdate, Playlist, Song},
    queue::Queue,
};
use egui::{include_image, Button, FontData, FontDefinitions, Ui, Widget};

const BUTTON_CORNER_RADIUS: u8 = 10;
const BUTTON_SPACING: f32 = 5.;

struct SeekBar<'a> {
    player: &'a mut Player,
}

impl<'a> SeekBar<'a> {
    fn new(player: &'a mut Player) -> Self {
        Self { player }
    }
}

fn format_time(mut secs: u32, show_hours: bool) -> String {
    let mut minutes = secs / 60;
    secs %= 60;
    if show_hours {
        let hours = minutes / 60;
        minutes %= 60;
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

impl Widget for &mut SeekBar<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            let time_playing = self.player.time_playing();
            let current_duration = match self.player.current() {
                Some(song) => *song.duration(),
                None => Duration::ZERO,
            };
            let mut percent = if current_duration.is_zero() {
                0.
            } else {
                time_playing.as_secs_f64() / current_duration.as_secs_f64()
            };
            let show_hours = current_duration.as_secs_f32() as u32 / 3600 > 0;
            let slider = egui::Slider::new(&mut percent, 0f64..=1f64).show_value(false);
            ui.style_mut().spacing.slider_width = ui.available_width();
            let resp = ui.add(slider);
            if resp.drag_stopped() {
                let seek_dur = current_duration.mul_f64(percent);
                time_playing.set_millis(seek_dur.as_millis() as u64);
                let _ = self.player.seek_duration(seek_dur);
            }
            let hover_pos = resp.hover_pos();
            let rect_left = resp.rect.left();
            let rect_width = resp.rect.width();
            resp.on_hover_ui_at_pointer(|ui| {
                // For some reason the hover_pos x can be lower than rect.left() or higher than rect.right() so I have to clamp it.
                let mouse_x = hover_pos.unwrap().x - rect_left;
                let percent = (mouse_x / rect_width).clamp(0., 1.);
                let hovered_time = current_duration.mul_f64(percent as f64);
                ui.label(format_time(hovered_time.as_secs_f32() as u32, show_hours));
            });
            let ctx = ui.ctx();
            if !ctx.has_requested_repaint() && !self.player.is_paused() {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        })
        .response
    }
}

struct PlaylistButton<'a> {
    playlist: &'a Playlist,
    height: f32,
    selected: bool,
}

impl<'a> PlaylistButton<'a> {
    fn new(playlist: &'a Playlist, height: f32, selected: bool) -> Self {
        Self {
            playlist,
            height,
            selected,
        }
    }
}

impl Widget for PlaylistButton<'_> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        // Somehow this horizontal wrapper fixes a bug with setting ui stroke color, I can't explain
        ui.vertical(|ui| {
            let mut size = ui.available_size();
            size.y = self.height;
            let image = if !self.playlist.exists() {
                ui.style_mut().visuals.selection.stroke.color =
                    egui::Color32::from_rgb(255, 165, 0);
                Some(
                    egui::Image::new(include_image!("../assets/button_icons/warning.svg"))
                        .max_size(egui::Vec2::new(50., 50.)),
                )
            } else {
                None
            };
            let button = Button::opt_image_and_text(image, Some(self.playlist.name().into()))
                .min_size(size)
                .selected(self.selected || !self.playlist.exists())
                .corner_radius(BUTTON_CORNER_RADIUS);
            ui.add(button)
        })
        .inner
    }
}

struct SongButton<'a> {
    song: &'a Song,
    height: f32,
    selected: bool,
}

impl<'a> SongButton<'a> {
    fn new(song: &'a Song, height: f32, selected: bool) -> Self {
        Self {
            song,
            height,
            selected,
        }
    }
}

impl Widget for SongButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut size = ui.available_size();
        size.y = self.height;
        let button = Button::new(self.song.title())
            .min_size(size)
            .selected(self.selected)
            .corner_radius(BUTTON_CORNER_RADIUS);
        ui.add(button)
    }
}

struct CenterControls<'a> {
    player: &'a mut Player,
}

impl<'a> CenterControls<'a> {
    fn new(player: &'a mut Player) -> Self {
        Self { player }
    }
}

impl Widget for &mut CenterControls<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            let rewind_button = Button::image(include_image!("../assets/button_icons/rewind.svg"))
                .corner_radius(BUTTON_CORNER_RADIUS);
            let size = (50., 50.);
            const NUM_BUTTONS: f32 = 3.;
            let spacing = &mut ui.spacing_mut().item_spacing.x;
            *spacing = 20.;
            let width = size.0 * NUM_BUTTONS + *spacing * (NUM_BUTTONS - 1.);
            ui.add_space((ui.available_width() - width) * 0.5);
            if ui.add_sized(size, rewind_button).clicked() {
                self.player.rewind();
            }
            let img = if self.player.is_paused() {
                include_image!("../assets/button_icons/resume.svg")
            } else {
                include_image!("../assets/button_icons/pause.svg")
            };
            let pause_button = Button::image(img).corner_radius(BUTTON_CORNER_RADIUS);
            if ui.add_sized(size, pause_button).clicked() {
                if self.player.is_paused() {
                    self.player.resume();
                } else {
                    self.player.pause();
                }
            }
            let ff_button =
                Button::image(include_image!("../assets/button_icons/fast-forward.svg"))
                    .corner_radius(BUTTON_CORNER_RADIUS);
            if ui.add_sized(size, ff_button).clicked() {
                self.player.fast_forward();
            }
        })
        .response
    }
}

#[derive(Clone, Debug)]
struct UiPlaylistInfo {
    selected: Option<(usize, Vec<Song>)>,
    active: Option<(usize, usize)>,
}

struct AmuseingApp {
    player: Player,
    config: Config,
    ui_playlist_info: UiPlaylistInfo,
    player_update: Option<Receiver<PlayerUpdate>>,
}

impl AmuseingApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.style_mut(|style| {
            use egui::{Color32, CornerRadius};
            let corner_radius = CornerRadius::same(0);
            style.visuals.widgets.inactive.corner_radius = corner_radius;
            style.visuals.widgets.active.corner_radius = corner_radius;
            style.visuals.widgets.hovered.corner_radius = corner_radius;
            style.visuals.selection.bg_fill = style.visuals.widgets.inactive.bg_fill;
            style.visuals.selection.stroke.color = Color32::from_gray(255);
        });
        cc.egui_ctx.set_theme(egui::Theme::Dark);

        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "Noto Sans".to_string(),
            std::sync::Arc::new(FontData::from_static(include_bytes!(
                "../assets/fonts/NotoSans-Regular.ttf"
            ))),
        );

        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Noto Sans".to_owned());
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.style_mut(|style| {
            style
                .text_styles
                .get_mut(&egui::TextStyle::Body)
                .unwrap()
                .size = 16.;
        });
        let config = Config::default();
        let playlist = &config.playlists[0];
        let songs = playlist.songs().unwrap();
        let ui_playlist_info = UiPlaylistInfo {
            selected: None,
            active: Some((0, 0)),
        };
        let mut player = Player::new(config.player.volume);
        {
            let mut queue = player.queue_mut();
            *queue = Queue::new(amuseing::queue::RepeatMode::All);
            queue.extend(songs.into_iter());
        }
        let player_update = player.run(config.player.buffer_size).ok();
        Self {
            player,
            config,
            ui_playlist_info,
            player_update,
        }
    }

    pub fn start_new_player(
        &mut self,
        songs: Vec<Song>,
        song_idx: usize,
    ) -> Result<(), PlayerStartError> {
        let new_player = Player::new(self.config.player.volume);
        let curr_repeat_mode = self.player.queue_mut().repeat_mode;
        {
            let mut queue = new_player.queue_mut();
            *queue = Queue::new(curr_repeat_mode);
            queue.extend(songs);
            queue
                .jump(song_idx)
                .expect("Should be able to jump to a song which is displayed in the ui");
        }
        self.player = new_player;
        let player_update = self.player.run(self.config.player.buffer_size);
        player_update.map(|update| {
            self.player_update = Some(update);
            ()
        })
    }

    fn try_start_new_player(
        &mut self,
        ui: &mut Ui,
        songs: Vec<Song>,
        playlist_idx: usize,
        song_idx: usize,
    ) {
        if songs.is_empty() {
            // FIXME: this shouldn't ever be possible
            return;
        }
        if let Err(e) = self.start_new_player(songs.clone(), song_idx) {
            let playlist = &self.config.playlists[playlist_idx];
            warn!(
                "Failed to start playlist '{}', error: {}",
                playlist.name(),
                e
            );
            //TODO: show popup with a display message saying yada yada
        } else {
            self.ui_playlist_info.active = Some((playlist_idx, song_idx));
        };
    }
}

impl eframe::App for AmuseingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let egui::Vec2 {
            x: window_width,
            y: window_height,
        } = ctx.screen_rect().size();
        let player = &mut self.player;
        if let Some(player_update) = &self.player_update {
            for message in player_update.try_iter() {
                match message {
                    PlayerUpdate::SongChange { song: _, index } => {
                        if let Some((_, active_song_id)) = self.ui_playlist_info.active.as_mut() {
                            // Yes I know this sets the active song ID twice when a song is clicked, whatcha gonna do about it
                            *active_song_id = index;
                        }
                    }
                    _ => {}
                }
            }
        }
        let controls_panel =
            egui::TopBottomPanel::bottom("Player controls panel").exact_height(100.);
        controls_panel.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let seek_bar = &mut SeekBar::new(player);
                ui.add_space(5.);
                ui.add(seek_bar);
                ui.add_space(5.);
                ui.columns_const(
                    |[song_display_ui, center_controls_ui, volume_controls_ui]| {
                        let mut center_controls = CenterControls::new(player);
                        center_controls_ui.add(&mut center_controls);
                    },
                );
            });
        });

        let playlist_panel_width = (window_width * 0.3).clamp(200., 500.);
        let playlist_panel = egui::SidePanel::left("Playlist tab")
            .exact_width(playlist_panel_width)
            .resizable(false);
        playlist_panel.show(ctx, |ui| {
            let total_rows = self.config.playlists.len();
            // const PLAYLISTS_SHOWN: f32 = 10.;
            // let row_height = ui.available_height() / PLAYLISTS_SHOWN;
            const ROW_HEIGHT: f32 = 80.;
            egui::ScrollArea::vertical().animated(true).show_rows(
                ui,
                ROW_HEIGHT,
                total_rows,
                |ui, row_range| {
                    let start = row_range.start;
                    ui.style_mut().spacing.item_spacing.y = BUTTON_SPACING;
                    for (i, playlist) in self.config.playlists[row_range].iter_mut().enumerate() {
                        let playlist_idx = i + start;
                        // Option::is_some_and would require a clone :(
                        let selected = if let Some((selected_playlist_id, _)) =
                            self.ui_playlist_info.selected
                        {
                            playlist_idx == selected_playlist_id
                        } else {
                            false
                        };
                        if ui
                            .add(PlaylistButton::new(&playlist, ROW_HEIGHT, selected))
                            .clicked()
                        {
                            if !playlist.check_exists() {
                                warn!(
                                    "Tried to select playlist '{}' with invalid path '{}'",
                                    playlist.name(),
                                    playlist.path().display()
                                );
                                continue;
                            }
                            self.ui_playlist_info.selected =
                                playlist.songs().ok().map(|songs| (playlist_idx, songs));
                            if self.ui_playlist_info.selected.is_none() {
                                egui::containers::popup::show_tooltip_at(
                                    ui.ctx(),
                                    egui::LayerId::new(
                                        egui::Order::Foreground,
                                        egui::Id::new("popup"),
                                    ),
                                    egui::Id::new("popup"),
                                    (window_width / 2., window_height / 2.).into(),
                                    |ui| {
                                        ui.label("kurcina");
                                    },
                                );
                            }
                        }
                    }
                },
            )
        });
        let central_panel = egui::CentralPanel::default();
        central_panel.show(ctx, |ui| {
            if let Some((selected_playlist_id, selected_songs)) =
                self.ui_playlist_info.selected.clone()
            {
                if selected_songs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("This playlist doesn't have any songs")
                    });
                } else {
                    let total_rows = selected_songs.len();
                    // const SONGS_SHOWN: f32 = 10.;
                    // let row_height = ui.available_height() / SONGS_SHOWN;
                    const ROW_HEIGHT: f32 = 60.;
                    egui::ScrollArea::vertical().animated(true).show_rows(
                        ui,
                        ROW_HEIGHT,
                        total_rows,
                        |ui, row_range| {
                            let start = row_range.start;
                            ui.style_mut().spacing.item_spacing.y = BUTTON_SPACING;
                            for (i, song) in selected_songs[row_range].iter().enumerate() {
                                let song_idx = i + start;
                                // let song_selected = self.active_playlist_id.is_some_and(|active_playlist_id| {
                                //     selected_playlist_id == active_playlist_id && song_idx == *song.id()
                                // });
                                let song_selected = self.ui_playlist_info.active.is_some_and(
                                    |(active_playlist_id, active_song_id)| {
                                        selected_playlist_id == active_playlist_id
                                            && song_idx == active_song_id
                                    },
                                );
                                // dbg!(song_idx, song.id());
                                let button_resp =
                                    ui.add(SongButton::new(&song, ROW_HEIGHT, song_selected));
                                if button_resp.clicked() {
                                    self.try_start_new_player(
                                        ui,
                                        selected_songs.clone(),
                                        selected_playlist_id,
                                        song_idx,
                                    );
                                }
                                button_resp.context_menu(|ui| {
                                    if ui.button("Play this song").clicked() {
                                        self.try_start_new_player(
                                            ui,
                                            selected_songs.clone(),
                                            selected_playlist_id,
                                            song_idx,
                                        );
                                    }
                                });
                            }
                        },
                    );
                }
            } else {
                ui.centered_and_justified(|ui| ui.label("No playlist selected"));
            }
        });
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Default, Debug)]
enum LogLevel {
    // Show all debug information
    Debug,
    // Show info, warnings, and errors
    Info,
    // Show warnings and errors
    Warn,
    // Show only errors
    #[default]
    Error,
}

impl LogLevel {
    fn to_level_filter(&self) -> log::LevelFilter {
        match self {
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
        }
    }
}

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long)]
    log: Option<LogLevel>,
    #[arg(long)]
    liblog: Option<LogLevel>,
}

fn main() {
    let args = Args::parse();
    let lib_log_level = args.liblog.unwrap_or_default().to_level_filter();
    let log_level = args.log.unwrap_or_default().to_level_filter();
    env_logger::Builder::new()
        .filter_level(lib_log_level)
        .filter_module("amuseing", log_level)
        .init();

    info!("Starting app");

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default()
        .with_min_inner_size((600., 400.))
        .with_title("amuseing")
        .with_resizable(true);
    native_options.renderer = eframe::Renderer::Wgpu;
    eframe::run_native(
        "Amuseing",
        native_options,
        Box::new(|cc| Ok(Box::new(AmuseingApp::new(cc)))),
    )
    .unwrap();

    info!("Exiting");
}
