//! AcidSynth's sound design panel. Displays are computed from the patch being edited.
use super::acid_synth::apply_preset;
use super::{ACID, App, BORDER, DIM, INK, Ui, egui};
use crate::midi::MidiMenu;
use egui::{Color32, Frame, Margin, Pos2, RichText, Sense, Stroke, Vec2};
use orb_engine::studio::acid_presets::PRESETS;
use orb_engine::studio::acid_synth::{Patch, Slot};
use std::f32::consts::{PI, TAU};

const PANEL: Color32 = Color32::from_rgb(28, 31, 32);
const SCREEN: Color32 = Color32::from_rgb(13, 17, 18);
const TEAL: Color32 = Color32::from_rgb(115, 209, 193);
const OCHRE: Color32 = Color32::from_rgb(237, 178, 112);
const LILAC: Color32 = Color32::from_rgb(183, 163, 226);

/// Draw a control inside the currently scoped module, with its own accessible keyboard target.
fn dial(ui: &mut Ui, label: &str, value: &mut f32, ink: Color32, midi: Option<&str>) -> bool {
    let initial = *value;
    ui.vertical_centered(|ui| {
        let size = ui.available_width().clamp(32.0, 46.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
        let response = if let Some(id) = midi { response.midi_menu(ui, id) } else { response };
        if response.clicked() { response.request_focus(); }
        if response.dragged_by(egui::PointerButton::Primary) {
            let delta = ui.input(|i| {
                -i.pointer.delta().y / if i.modifiers.shift { 800.0 } else { 145.0 }
            });
            *value = (*value + delta).clamp(0.0, 1.0);
        }
        if response.has_focus() {
            ui.input(|i| {
                let up = i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight);
                let down = i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft);
                *value = (*value + (i32::from(up) - i32::from(down)) as f32 * 0.01).clamp(0.0, 1.0);
            });
        }
        let center = rect.center();
        let radius = size * 0.43;
        let position = |fraction: f32, r: f32| {
            let angle = (-0.75 + 1.5 * fraction) * PI;
            center + Vec2::new(angle.sin(), -angle.cos()) * r
        };
        let arc = |end: f32| (0..=32).map(|n| position(end * n as f32 / 32.0, radius)).collect::<Vec<_>>();
        let painter = ui.painter();
        painter.circle_filled(center, radius - 3.0, Color32::from_rgb(40, 44, 45));
        painter.add(egui::Shape::line(arc(1.0), Stroke::new(2.5, BORDER)));
        painter.add(egui::Shape::line(arc(*value), Stroke::new(2.5, ink)));
        painter.line_segment([position(*value, radius * 0.47), position(*value, radius * 0.74)], Stroke::new(2.0, INK));
        if response.has_focus() {
            painter.circle_stroke(center, radius + 3.0, Stroke::new(1.0, ink));
        }
        response.on_hover_text(format!("{label} · {:.0}%\nDrag or use arrow keys. Hold Shift for fine adjustment.", *value * 100.0));
        ui.label(RichText::new(label).size(11.0).color(INK));
    });
    *value != initial
}

fn dials(ui: &mut Ui, params: &mut [(&str, &mut f32)], ink: Color32) -> bool {
    let mut changed = false;
    ui.columns(params.len(), |cols| {
        for (col, (label, value)) in cols.iter_mut().zip(params) {
            changed |= dial(col, label, value, ink, None);
        }
    });
    changed
}

fn module(ui: &mut Ui, title: &str, accent: Color32, draw: impl FnOnce(&mut Ui)) {
    Frame::new().fill(PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(5.0)
        .inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.scope_builder(egui::UiBuilder::new().id_salt(title), |ui| {
                ui.label(RichText::new(title).monospace().strong().size(12.0).color(accent));
                ui.add_space(4.0);
                draw(ui);
            });
        });
}

fn graph(ui: &mut Ui, ink: Color32, f: impl Fn(f32) -> f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 51.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, SCREEN);
    let grid = Stroke::new(1.0, Color32::from_rgb(32, 40, 40));
    let plot = rect.shrink(3.0);
    for i in 1..8 {
        let x = plot.left() + plot.width() * i as f32 / 8.0;
        painter.line_segment([Pos2::new(x, plot.top()), Pos2::new(x, plot.bottom())], grid);
    }
    for i in 1..4 {
        let y = plot.top() + plot.height() * i as f32 / 4.0;
        painter.line_segment([Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)], grid);
    }
    let points = (0..=180).map(|i| {
        let t = i as f32 / 180.0;
        Pos2::new(plot.left() + plot.width()*t, plot.bottom() - plot.height()*f(t).clamp(0.0, 1.0))
    }).collect();
    painter.add(egui::Shape::line(points, Stroke::new(1.65, ink)));
}

fn morph(t: f32, wave: f32) -> f32 {
    let phase = TAU * t;
    let fundamental = phase.sin();
    let mut triangle = 0.0;
    let mut saw = 0.0;
    for h in 1..=17 {
        let sine = (phase * h as f32).sin();
        saw += sine * 0.57 / h as f32;
        if h % 2 == 1 {
            triangle += sine * if h % 4 == 1 {0.81} else {-0.81} / (h as f32).powi(2);
        }
    }
    let formant = 0.45 * fundamental + 0.15 * (3.0*phase).sin() + 0.24 * (7.0*phase).sin()
        + 0.16 * (11.0*phase).sin() + 0.12 * (17.0*phase).sin();
    let table = [fundamental, triangle, saw, formant];
    let p = wave * 3.0;
    let index = (p as usize).min(2);
    table[index] + (table[index+1] - table[index]) * (p - index as f32)
}

fn oscillator_graph(ui: &mut Ui, patch: &Patch) {
    graph(ui, TEAL, |x| {
        let a = morph(x * 2.0, patch.wave_a);
        let b = morph(x * 2.0 * 2.0_f32.powf(f32::from(patch.transpose_b) / 12.0), patch.wave_b);
        let osc = match patch.osc_mode {
            1 => a + (1.6 * a * b - a) * patch.mix,
            2 => morph(x * 2.0 + 0.22 * b * patch.mix, patch.wave_a),
            _ => a + (b - a) * patch.mix,
        };
        osc * 0.44 + 0.5
    });
}

fn envelope_graph(ui: &mut Ui, p: &Patch) {
    let attack = 0.004 + 3.0 * p.attack.powi(2);
    let decay = 0.012 + 2.5 * p.decay.powi(2);
    let release = 0.025 + 5.0 * p.release.powi(2);
    let held = attack + decay + 0.45;
    graph(ui, OCHRE, |x| {
        let t = x * (held + release);
        let amp = if t < attack { t / attack } else if t < attack + decay {
            p.sustain + (1.0 - p.sustain) * (-6.91 * (t - attack) / decay).exp()
        } else if t < held { p.sustain } else {
            p.sustain * (-6.91 * (t - held) / release).exp()
        };
        0.04 + amp * 0.91
    });
}

fn filter_graph(ui: &mut Ui, p: &Patch) {
    let cutoff = 70.0 * 2.0_f32.powf(7.0 * p.cutoff);
    let damping = 1.9 - 1.65 * p.resonance;
    graph(ui, ACID, |x| {
        let ratio = 30.0 * 2.0_f32.powf(9.5 * x) / cutoff;
        let sq = ratio*ratio;
        let gain = match p.filter_mode {
            1 => ratio,
            2 => sq,
            3 => (1.0-sq).abs(),
            _ => 1.0,
        } / ((1.0-sq).powi(2) + (damping*ratio).powi(2)).sqrt();
        (gain.max(0.001).log10() * 20.0 + 44.0) / 58.0
    });
}

fn lfo_graph(ui: &mut Ui, p: &Patch) {
    graph(ui, LILAC, |x| {
        let phase = (x * (1.0 + p.lfo_rate * 4.0)).fract();
        let v = match p.lfo_shape {
            1 => 1.0 - 4.0*(phase-0.5).abs(),
            2 => 2.0*phase - 1.0,
            3 => if phase < 0.5 { 1.0 } else { -1.0 },
            _ => (TAU*phase).sin(),
        };
        0.5 + v * (p.lfo_depth * 0.44)
    });
}

fn modes(ui: &mut Ui, label: &str, value: &mut u8, names: &[&str]) -> bool {
    let old = *value;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).size(10.0).color(DIM));
        for (i, &name) in names.iter().enumerate() {
            ui.selectable_value(value, i as u8, name);
        }
    });
    *value != old
}

fn matrix(ui: &mut Ui, p: &mut Patch) {
    const SOURCES: [&str; 7] = ["Off", "LFO", "Amp env", "Timbre", "Movement", "Width", "Space"];
    const DESTINATIONS: [&str; 6] = ["Wave A", "Wave B", "Cutoff", "Balance", "Pitch", "Drive"];
    let active = p.routes.iter().filter(|route| route.source != 0 && route.amount != 0.0).count();
    egui::CollapsingHeader::new(RichText::new(format!("Modulation matrix   {active} / 4")).monospace().color(LILAC))
        .show(ui, |ui| {
            egui::Grid::new("patch-modulation-routes").striped(true).min_col_width(80.0)
                .show(ui, |ui| {
                    for label in ["Source", "Destination", "Amount"] { ui.small(label); }
                    ui.end_row();
                    for (i, route) in p.routes.iter_mut().enumerate() {
                        egui::ComboBox::from_id_salt(("route-src", i))
                            .selected_text(SOURCES[usize::from(route.source).min(6)])
                            .show_ui(ui, |ui| {
                                for (n, &name) in SOURCES.iter().enumerate() {
                                    ui.selectable_value(&mut route.source, n as u8, name);
                                }
                            });
                        egui::ComboBox::from_id_salt(("route-dst", i))
                            .selected_text(DESTINATIONS[usize::from(route.dest).min(5)])
                            .show_ui(ui, |ui| {
                                for (n, &name) in DESTINATIONS.iter().enumerate() {
                                    ui.selectable_value(&mut route.dest, n as u8, name);
                                }
                            });
                        ui.add(egui::Slider::new(&mut route.amount, -1.0..=1.0));
                        ui.end_row();
                    }
                });
        });
}

fn patch_panel(ui: &mut Ui, slot: &mut Slot, row: usize) -> bool {
    let before = slot.patch;
    let initial_macros = slot.macros;
    let initial_preset = slot.preset;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("AcidSynth").size(25.0).strong().color(INK));
        ui.label(RichText::new(["A", "B"][row]).monospace().color(TEAL));
        ui.add_space(12.0);
        let current = usize::from(slot.preset).min(PRESETS.len()-1);
        let suffix = if PRESETS[current].1 == slot.patch { "" } else { " *" };
        egui::ComboBox::from_id_salt(("sound-preset", row)).width(195.0)
            .selected_text(format!("{}{suffix}", PRESETS[current].0)).show_ui(ui, |ui| {
                for (i, (name, _)) in PRESETS.iter().enumerate() {
                    if ui.selectable_label(i == current, *name).clicked() {
                        apply_preset(slot, i);
                    }
                }
            });
    });
    ui.add_space(5.0);
    let p = &mut slot.patch;
    ui.columns(2, |cols| {
        module(&mut cols[0], "OSCILLATORS", TEAL, |ui| {
            oscillator_graph(ui, p);
            modes(ui, "ENGINE", &mut p.osc_mode, &["Blend", "Ring", "Phase mod"]);
            dials(ui, &mut [("Table A", &mut p.wave_a), ("Table B", &mut p.wave_b), ("Balance", &mut p.mix), ("Detune", &mut p.detune)], TEAL);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Osc B pitch").size(11.0).color(DIM));
                ui.add(egui::DragValue::new(&mut p.transpose_b).range(-24..=24).suffix(" st"));
            });
        });
        module(&mut cols[1], "MULTIMODE FILTER", ACID, |ui| {
            filter_graph(ui, p);
            modes(ui, "MODE", &mut p.filter_mode, &["LP", "BP", "HP", "Notch"]);
            dials(ui, &mut [("Cutoff", &mut p.cutoff), ("Resonance", &mut p.resonance), ("Env depth", &mut p.filter_env)], ACID);
        });
    });
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        module(&mut cols[0], "AMPLITUDE ENVELOPE", OCHRE, |ui| {
            envelope_graph(ui, p);
            dials(ui, &mut [("Attack", &mut p.attack), ("Decay", &mut p.decay), ("Sustain", &mut p.sustain), ("Release", &mut p.release)], OCHRE);
        });
        module(&mut cols[1], "MODULATION", LILAC, |ui| {
            lfo_graph(ui, p);
            modes(ui, "WAVE", &mut p.lfo_shape, &["Sine", "Tri", "Ramp", "Square"]);
            modes(ui, "TO", &mut p.lfo_target, &["Colour", "Filter", "Pitch", "Mix"]);
            dials(ui, &mut [("Rate", &mut p.lfo_rate), ("Depth", &mut p.lfo_depth)], LILAC);
        });
    });
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        module(&mut cols[0], "CHARACTER", TEAL, |ui| {
            dials(ui, &mut [("Sub", &mut p.sub), ("Drive", &mut p.drive), ("Chorus", &mut p.chorus)], TEAL);
        });
        module(&mut cols[1], "PERFORMANCE MACROS", ACID, |ui| {
            ui.columns(4, |cols| {
                for (i, (key, title)) in [("timbre","Timbre"),("movement","Movement"),("width","Width"),("space","Space")].into_iter().enumerate() {
                    let midi = format!("acidsynth{}.{key}", row+1);
                    dial(&mut cols[i], title, &mut slot.macros[i], ACID, Some(&midi));
                }
            });
        });
    });
    matrix(ui, &mut slot.patch);
    before != slot.patch || initial_macros != slot.macros || initial_preset != slot.preset
}

impl App {
    pub(super) fn acid_editor(&mut self, ui: &mut Ui, row: usize, id: egui::Id) {
        let mut open = true;
        let mut changed = false;
        let slot = &mut self.studio.settings.synth_slots[row];
        egui::Window::new(format!("AcidSynth {} / Sound editor", ["A","B"][row]))
            .open(&mut open).default_width(690.0).min_width(540.0).vscroll(true)
            .show(ui.ctx(), |ui| {
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(56, 74, 71);
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, TEAL);
                changed = patch_panel(ui, slot, row);
            });
        if changed {
            self.studio.changed();
            self.doc.modified = true;
        }
        if !open {
            ui.ctx().data_mut(|d| d.insert_temp(id, None::<usize>));
        }
    }
}
