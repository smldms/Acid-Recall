//! Persistent audio preferences, separate from MIDI and portable songs.

use eframe::egui::{self, Context};
use orb_player::output::{self, Config};

use crate::app::App;

#[derive(Default)]
pub struct Preferences {
    pub open: bool,
    pub draft: Config,
    notice: Option<String>,
}

impl App {
    pub(crate) fn audio_preferences_ui(&mut self, ctx: &Context) {
        if !self.audio_preferences.open {
            return;
        }
        let mut open = true;
        let mut refresh = false;
        let mut apply = false;
        let mut reset = false;
        let previous_backend = self.audio_preferences.draft.backend.clone();
        let catalog = self
            .audio_session
            .as_ref()
            .and_then(|a| a.catalog(&self.audio_preferences.draft.backend))
            .cloned();
        let active = self
            .audio_session
            .as_ref()
            .and_then(|a| a.stream())
            .map(|s| {
                (
                    s.backend.clone(),
                    s.device.clone(),
                    s.config.sample_rate,
                    s.config.channels,
                    s.callback_frames(),
                    s.render_timing(),
                    s.resampled,
                    s.failure(),
                )
            });
        let window = egui::Window::new("Preferences / Audio")
            .id(egui::Id::unique("audio-preferences"))
            .open(&mut open)
            .default_width(520.0)
            .resizable(true);
        window.show(ctx, |ui| {
            ui.heading("AUDIO OUTPUT");
            ui.label("Choose the audio driver, device and stereo outputs.");
            ui.small(
                "Changing backend, refreshing devices or applying settings stops playback. \
                    Your song and samples stay loaded.",
            );
            ui.separator();
            let draft = &mut self.audio_preferences.draft;
            let default = output::default_backend();
            let backend_label = if draft.backend.is_empty() {
                format!("System default ({default})")
            } else {
                draft.backend.clone()
            };
            ui.horizontal(|ui| {
                ui.label("Driver");
                egui::ComboBox::from_id_salt("audio-backend")
                    .selected_text(backend_label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut draft.backend,
                            String::new(),
                            format!("System default ({default})"),
                        );
                        for backend in output::backends() {
                            ui.selectable_value(&mut draft.backend, backend.clone(), backend);
                        }
                    });
                refresh = ui.button("Refresh devices").clicked();
            });
            let selected_device = catalog.as_ref().and_then(|c| {
                let id = draft.device.as_ref().or(c.default_device.as_ref());
                c.devices.iter().find(|d| Some(&d.id) == id)
            });
            let device_label = if draft.device.is_none() {
                "Default output".to_owned()
            } else {
                selected_device.map_or_else(|| "Unavailable output".into(), |d| d.name.clone())
            };
            let old_device = draft.device.clone();
            ui.horizontal(|ui| {
                ui.label("Device");
                egui::ComboBox::from_id_salt("audio-device")
                    .selected_text(device_label)
                    .width(350.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.device, None, "Default output");
                        if let Some(c) = &catalog {
                            for device in &c.devices {
                                ui.selectable_value(
                                    &mut draft.device,
                                    Some(device.id.clone()),
                                    &device.name,
                                );
                            }
                        }
                    });
            });
            if old_device != draft.device {
                draft.output_pair = 0;
                draft.sample_rate = None;
                draft.buffer_frames = None;
            }
            let pairs = selected_device.map_or(1, |d| (d.channels / 2).max(1));
            ui.horizontal(|ui| {
                ui.label("Stereo outputs");
                egui::ComboBox::from_id_salt("audio-outputs")
                    .selected_text(format!(
                        "{} / {}",
                        draft.output_pair + 1,
                        draft.output_pair + 2
                    ))
                    .show_ui(ui, |ui| {
                        for pair in 0..pairs {
                            ui.selectable_value(
                                &mut draft.output_pair,
                                pair * 2,
                                format!("{} / {}", pair * 2 + 1, pair * 2 + 2),
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Sample rate");
                egui::ComboBox::from_id_salt("audio-rate")
                    .selected_text(draft.sample_rate.map_or_else(
                        || "Automatic (44.1 kHz preferred)".into(),
                        |r| format!("{r} Hz"),
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut draft.sample_rate,
                            None,
                            "Automatic (44.1 kHz preferred)",
                        );
                        if let Some(device) = selected_device {
                            for &rate in &device.rates {
                                ui.selectable_value(
                                    &mut draft.sample_rate,
                                    Some(rate),
                                    format!("{rate} Hz"),
                                );
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Buffer size");
                egui::ComboBox::from_id_salt("audio-buffer")
                    .selected_text(
                        draft
                            .buffer_frames
                            .map_or_else(|| "Driver default".into(), |n| format!("{n} frames")),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.buffer_frames, None, "Driver default");
                        for frames in [32, 64, 128, 256, 512, 1024, 2048, 4096, 8192] {
                            if selected_device
                                .and_then(|d| d.buffer_range)
                                .is_none_or(|(min, max)| (min..=max).contains(&frames))
                            {
                                ui.selectable_value(
                                    &mut draft.buffer_frames,
                                    Some(frames),
                                    format!("{frames} frames"),
                                );
                            }
                        }
                    });
            });
            ui.small(
                "If a driver rejects a fixed buffer size, \
                    use Driver default or its own control panel.",
            );
            if let Some(c) = &catalog
                && c.devices.is_empty()
            {
                ui.label(
                    "No usable outputs found. Check the installed driver, \
                        then refresh devices.",
                );
            }
            ui.separator();
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(self.audio_session.is_some(), egui::Button::new("APPLY"))
                    .clicked();
                reset = ui.button("Reset to system default").clicked();
            });
            if let Some(message) = &self.audio_preferences.notice {
                ui.label(message);
            }
            ui.separator();
            ui.strong("Currently active");
            if let Some((backend, name, rate, channels, frames, (peak, late), resampled, error)) = &active {
                ui.label(format!("{backend} / {name}"));
                ui.label(format!("{rate} Hz / {channels} channels"));
                if *frames > 0 {
                    ui.label(format!(
                        "Observed buffer: {frames} frames (~{:.1} ms)",
                        *frames as f64 * 1000.0 / f64::from(*rate)
                    ));
                }
                ui.small("Time shown is one buffer, not total audio or MIDI latency.");
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Peak render load {peak:.1}%  /  buffers over budget {late}"))
                        .on_hover_text("Above 100%, rendering took longer than the available buffer duration.");
                    if ui.small_button("Reset counters").clicked()
                        && let Some(stream) = self.audio_session.as_ref().and_then(|a| a.stream())
                    {
                        stream.reset_render_timing();
                    }
                });
                if *resampled {
                    ui.small("The 44.1 kHz engine is resampled for this output.");
                }
                if let Some(error) = error {
                    ui.label(format!("Output error: {error}"));
                }
            } else if self.audio_session.is_none() {
                ui.label("Audio is disabled in this demo/test view.");
            } else {
                ui.label("No active output. Choose a device and Apply to reconnect.");
            }
        });
        if previous_backend != self.audio_preferences.draft.backend {
            self.audio_preferences.draft.device = None;
            self.audio_preferences.draft.sample_rate = None;
            self.audio_preferences.draft.buffer_frames = None;
            self.audio_preferences.draft.output_pair = 0;
            refresh = true;
        }
        if reset {
            self.audio_preferences.draft = Config::default();
            refresh = true;
        }
        if (refresh || apply) && self.audio_session.is_some() {
            self.pause_midi_for_audio_change();
        }
        if let Some(session) = &mut self.audio_session {
            if refresh {
                self.audio_preferences.notice = Some(
                    match session.refresh(&self.audio_preferences.draft.backend) {
                        Ok(()) => "Audio devices refreshed; playback stopped.".into(),
                        Err(e) => e.to_string(),
                    },
                );
            }
            if apply {
                match session.apply(&self.audio_preferences.draft) {
                    Ok(()) => {
                        self.settings.audio = self.audio_preferences.draft.clone();
                        self.audio_preferences.notice = Some(match self.settings.save() {
                            Ok(()) => "Audio settings applied and saved. Playback stopped.".into(),
                            Err(e) => format!("Audio applied; preferences could not save: {e}"),
                        });
                    }
                    Err(e) => self.audio_preferences.notice = Some(e.to_string()),
                }
            }
            self.audio_health = session.stream().map(|s| s.health());
            if refresh || apply {
                self.audio_reported = false;
            }
        }
        self.audio_preferences.open = open;
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_preferences_show_driver_routing_rate_and_buffer_without_modifying_saved_settings() {
        fn collect(shape: &egui::Shape, labels: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => labels.push(text.galley.text().into()),
                egui::Shape::Vec(items) => {
                    for shape in items {
                        collect(shape, labels);
                    }
                }
                _ => {}
            }
        }
        let ctx = Context::default();
        let mut app = App::new(crate::model::Panel::new(), None);
        app.audio_preferences.open = true;
        app.audio_preferences.draft.backend = "ASIO".into();
        let saved = app.settings.audio.clone();
        let mut warm = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| app.audio_preferences_ui(ui.ctx()),
        );
        warm.textures_delta.clear();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| app.audio_preferences_ui(ui.ctx()),
        );
        out.textures_delta.clear();
        let mut labels = Vec::new();
        for shape in &out.shapes {
            collect(&shape.shape, &mut labels);
        }
        for expected in [
            "Driver",
            "Device",
            "Stereo outputs",
            "Sample rate",
            "Buffer size",
            "ASIO",
            "APPLY",
        ] {
            assert!(
                labels.iter().any(|label| label == expected),
                "missing {expected}"
            );
        }
        assert_eq!(app.settings.audio, saved);
    }
}
