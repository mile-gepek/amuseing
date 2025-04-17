use core::f32;
use std::{path::PathBuf, time::Duration};

use amuseing::{config::Config, playback::Player, queue::Queue};
use egui::{include_image, Button, FontData, FontDefinitions, Widget};

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
            let time_playing_label =
                egui::Label::new(format_time(time_playing.as_secs_f64() as u32, show_hours));
            let duration_label = egui::Label::new(format_time(
                current_duration.as_secs_f64() as u32,
                show_hours,
            ));
            let label_size = (60., ui.available_height());
            let width = ui.available_width();
            let slider_width = ui.available_width() * 0.5;
            let item_spacing_x = ui.spacing().item_spacing.x;
            // let slider_width = ui.available_width() - 2. * label_size.0 - 2. * item_spacing_x;
            ui.style_mut().spacing.slider_width = slider_width;
            let space_leftover = width - slider_width - 2. * label_size.0 - 2. * item_spacing_x;
            ui.add_space(space_leftover * 0.5);
            ui.add_sized(label_size, time_playing_label);
            let slider = egui::Slider::new(&mut percent, 0f64..=1f64).show_value(false);
            let resp = ui.add(slider);
            ui.add(duration_label);
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
            let rewind_button = Button::image("../assets/button_icons/resume.svg");
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
    player: Option<Player>,
    config: Config,
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
        let mut queue = Queue::new(amuseing::queue::RepeatMode::All);
        queue.extend(songs.into_iter());
        let mut player = Player::with_queue(queue, config.player.volume);
        let _ = player.run(config.player.buffer_size);
        Self {
            player: Some(player),
            config,
        }
    }
}

impl eframe::App for AmuseingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(player) = self.player.as_mut() {
            let controls_panel =
                egui::TopBottomPanel::bottom("Player controls panel").exact_height(105.);
            controls_panel.show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(15.);
                    let seek_bar = &mut SeekBar::new(player);
                    ui.add(seek_bar);
                    ui.columns_const(|[song_display_ui, center_controls_ui, volume_controls]| {
                        let mut center_controls = CenterControls::new(player);
                        center_controls_ui.add(&mut center_controls);
                    });
                });
            });
        }
        let playlist_panel =
            egui::SidePanel::left("Playlist tab").width_range(egui::Rangef::new(300., 500.));
        playlist_panel.show(ctx, |ui| {
            for playlist in self.config.playlists.iter() {
                let _ = ui.button(playlist.name());
            }
        });
        let central_panel = egui::CentralPanel::default();
        central_panel.show(ctx, |ui| {});
    }
}

fn main() {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport.resizable = Some(true);
    eframe::run_native(
        "Amuseing",
        native_options,
        Box::new(|cc| Ok(Box::new(AmuseingApp::new(cc)))),
    )
    .unwrap();
}
