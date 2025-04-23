use std::{sync::mpsc::Receiver, time::Duration};

use amuseing::{
    config::Config,
    errors::PlayerStartError,
    playback::{Player, PlayerUpdate, Song},
    queue::Queue,
};
use egui::{include_image, Button, FontData, FontDefinitions, Ui, Widget};

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

struct SongButton<'a>(&'a Song);

impl Widget for SongButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let size = ui.available_size();
        let button = Button::new(self.0.title()).min_size(size);
        dbg!(size);
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
            let rewind_button = Button::image(include_image!("../assets/button_icons/rewind.svg"));
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
            let pause_button = Button::image(img);
            if ui.add_sized(size, pause_button).clicked() {
                if self.player.is_paused() {
                    self.player.resume();
                } else {
                    self.player.pause();
                }
            }
            let ff_button =
                Button::image(include_image!("../assets/button_icons/fast-forward.svg"));
            if ui.add_sized(size, ff_button).clicked() {
                self.player.fast_forward();
            }
        })
        .response
    }
}

struct AmuseingApp {
    player: Player,
    config: Config,
    selected_playlist_songs: Option<Vec<Song>>,
    player_update: Option<Receiver<PlayerUpdate>>,
}

impl AmuseingApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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
        let selected_playlist_songs = songs.clone();
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
            selected_playlist_songs: Some(selected_playlist_songs),
            player_update,
        }
    }

    pub fn start_new_player(
        &mut self,
        songs: Vec<Song>,
        song_idx: usize,
    ) -> Result<(), PlayerStartError> {
        let mut new_player = Player::new(self.config.player.volume);
        let curr_repeat_mode = self.player.queue_mut().repeat_mode;
        {
            let mut queue = new_player.queue_mut();
            *queue = Queue::new(curr_repeat_mode);
            queue.extend(songs);
            queue
                .jump(song_idx)
                .expect("Should be able to jump to a song which is displayed in the ui");
        }
        let player_update = new_player.run(self.config.player.buffer_size)?;
        self.player_update = Some(player_update);
        self.player = new_player;
        Ok(())
    }

    fn try_start_new_player(&mut self, ui: &mut Ui, songs: Vec<Song>, idx: usize) {
        if self.start_new_player(songs.clone(), idx).is_err() {
            //TODO: show popup with a display message saying yada yada
        };
    }
}

impl eframe::App for AmuseingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let player = &mut self.player;
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
        let playlist_panel =
            egui::SidePanel::left("Playlist tab").width_range(egui::Rangef::new(300., 500.));
        playlist_panel.show(ctx, |ui| {
            for playlist in self.config.playlists.iter() {
                let _ = ui.button(playlist.name());
            }
        });
        let central_panel = egui::CentralPanel::default();
        central_panel.show(ctx, |ui| {
            if let Some(songs) = self.selected_playlist_songs.clone() {
                let total_rows = songs.len();
                const SONGS_SHOWN: f32 = 10.;
                let row_height = ui.available_height() / SONGS_SHOWN as f32;
                dbg!(row_height);
                egui::ScrollArea::vertical().animated(true).show_rows(
                    ui,
                    row_height,
                    total_rows,
                    |ui, row_range| {
                        for (idx, song) in songs[row_range].iter().enumerate() {
                            let button_resp = ui.add(SongButton(&song));
                            if button_resp.clicked() {
                                self.try_start_new_player(ui, songs.clone(), idx);
                            }
                            button_resp.context_menu(|ui| {
                                if ui.button("Play this song").clicked() {
                                    self.try_start_new_player(ui, songs.clone(), idx);
                                }
                            });
                        }
                    },
                );
            } else {
                ui.centered_and_justified(|ui| ui.label("No playlist selected"));
            }
        });
    }
}
fn main() {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport.resizable = Some(true);
    native_options.renderer = eframe::Renderer::Wgpu;
    eframe::run_native(
        "Amuseing",
        native_options,
        Box::new(|cc| Ok(Box::new(AmuseingApp::new(cc)))),
    )
    .unwrap();
}
