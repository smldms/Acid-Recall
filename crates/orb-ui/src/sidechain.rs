//! Additional mix settings, persisted locally and included in audio exports.

use eframe::egui::{self, Align2, Color32, Context, FontId, Sense, Ui};
use orb_engine::sidechain::{Config, Source};
use orb_player::Command;
use serde::{Deserialize, Serialize};

use crate::app::{App, View};
use crate::model::Event;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub source: u8,
    pub depth: [u8; 2],
    pub release_ms: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            source: 2,
            depth: [55, 55],
            release_ms: 180,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.source = self.source.min(2);
        self.depth = self.depth.map(|v| v.min(100));
        self.release_ms = self.release_ms.clamp(20, 1000);
    }

    pub fn engine_config(&self) -> Config {
        Config {
            enabled: self.enabled,
            source: match self.source {
                0 => Source::Tr808,
                1 => Source::Tr909,
                _ => Source::Both,
            },
            depth: self.depth,
            release_ms: self.release_ms,
        }
        .sanitized()
    }
}

pub struct SidechainUi {
    pub open: bool,
    sent: Option<Config>,
    hat_sent: Option<Option<u8>>,
    save_at: Option<f64>,
}

impl Default for SidechainUi {
    fn default() -> Self {
        Self {
            open: false,
            // A fresh player already has these defaults. Avoid sending no-op commands
            // that would delay the first legacy snapshot behind a command barrier.
            sent: Some(Config::default()),
            hat_sent: Some(None),
            save_at: None,
        }
    }
}

impl App {
    /// CH edits use the normal mouse/MIDI panel route; these extensions are sent through
    /// the same ordered queue and snapshot barrier as the original controls.
    pub fn sync_effects(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        let level = self
            .panel
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Set { id, value } if id == "tr909.ch_level" => {
                    Some((*value).clamp(0, 127) as u8)
                }
                _ => None,
            });
        // Editing either hat opts into independence. Until then, legacy songs retain
        // their original shared-level behavior, including existing automation.
        let level =
            level.or_else(|| {
                (self.settings.closed_hat_level.is_none()
                    && self.panel.events.iter().any(
                        |event| matches!(event, Event::Set { id, .. } if id == "tr909.hh_level"),
                    ))
                .then(|| self.panel.value("tr909.ch_level").unwrap_or(100))
            });
        if let Some(level) = level
            && self.settings.closed_hat_level != Some(level)
        {
            self.settings.closed_hat_level = Some(level);
            self.sidechain.save_at = Some(now + 0.35);
        }
        if let Some(link) = &mut self.link {
            let config = self.settings.sidechain.engine_config();
            if self.sidechain.sent != Some(config) {
                link.effect_command(Command::Sidechain(config));
                self.sidechain.sent = Some(config);
            }
            if self.sidechain.hat_sent != Some(self.settings.closed_hat_level) {
                link.effect_command(Command::ClosedHatLevel(self.settings.closed_hat_level));
                self.sidechain.hat_sent = Some(self.settings.closed_hat_level);
            }
        }
        if let Some(level) = self.settings.closed_hat_level {
            self.panel.adopt("tr909.ch_level", i32::from(level));
        }
        if let Some(at) = self.sidechain.save_at {
            if (now >= at && !ctx.input(|i| i.pointer.any_down()))
                || ctx.input(|i| i.viewport().close_requested())
            {
                self.sidechain.save_at = None;
                if let Err(e) = self.settings.save() {
                    self.fail(format!("Cannot save mix settings: {e}"));
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }
    }

    pub(crate) fn sidechain_button(&mut self, ui: &mut Ui, view: &View) {
        if self.browsing() || self.prompting() || self.error_open() {
            return;
        }
        let rect = view.rect(454.0, 28.0, 18.0, 14.0);
        let response = ui.interact(rect, egui::Id::unique("kick-sidechain"), Sense::click());
        let fill = if self.settings.sidechain.enabled {
            Color32::from_rgb(0x4a, 0x53, 0x32)
        } else {
            Color32::from_rgb(0x30, 0x30, 0x34)
        };
        ui.painter().rect_filled(rect, 2.0 * view.scale, fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "SC",
            FontId::proportional(6.5 * view.scale),
            crate::palette::INK,
        );
        if response
            .on_hover_text("Kick sidechain: make room for the 808/909 kick")
            .clicked()
        {
            self.sidechain.open = true;
        }
    }

    pub(crate) fn sidechain_ui(&mut self, ctx: &Context) {
        if !self.sidechain.open {
            return;
        }
        let reduction = self
            .link
            .as_mut()
            .map(|l| l.player().snapshot().sidechain_reduction)
            .unwrap_or([0; 2]);
        let mut open = true;
        let mut changed = false;
        egui::Window::new("Kick Sidechain")
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                let s = &mut self.settings.sidechain;
                changed |= ui.checkbox(&mut s.enabled, "Enable kick ducking").changed();
                ui.label("Each kick briefly lowers the TBs, leaving room for its transient.");
                ui.horizontal(|ui| {
                    ui.label("Trigger");
                    for (source, label) in [(0, "808 kick"), (1, "909 kick"), (2, "Both")] {
                        changed |= ui.selectable_value(&mut s.source, source, label).changed();
                    }
                });
                for (row, label) in ["TB-303 A", "TB-303 B"].into_iter().enumerate() {
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut s.depth[row], 0..=100)
                                .text(format!("{label} depth"))
                                .suffix(" %"),
                        )
                        .changed();
                    ui.add(
                        egui::ProgressBar::new(f32::from(reduction[row]) / 100.0)
                            .text(format!("{label}: -{} %", reduction[row])),
                    );
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut s.release_ms, 20..=1000)
                            .text("Return")
                            .suffix(" ms"),
                    )
                    .changed();
                ui.small("Depth 0 leaves that TB unchanged. A smoothed attack avoids clicks.");
                ui.separator();
                ui.small("Saved with this app and included in audio exports.");
                ui.small("Standard RBS files do not store these additional mix settings.");
            });
        self.sidechain.open = open;
        if changed {
            self.settings.sidechain.sanitize();
            self.sidechain.save_at = Some(ctx.input(|i| i.time) + 0.35);
            self.sync_effects(ctx);
            ctx.request_repaint();
        }
        if self.settings.sidechain.enabled {
            ctx.request_repaint_after(crate::app::METER_FRAME);
        }
    }
}
