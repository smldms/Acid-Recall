//! AcidSynth's sound design panel. Displays are computed from the patch being edited.
use super::acid_synth::apply_preset;
use super::{ACID, App, BORDER, DIM, INK, Ui, egui};
use crate::midi::MidiMenu;
use egui::{Color32, Frame, Margin, Pos2, RichText, Sense, Stroke, Vec2};
use orb_engine::studio::acid_presets::{EXPERIMENTAL_START, PRESETS};
use orb_engine::studio::acid_synth::{Patch, Slot};
use std::f32::consts::{PI, TAU};

const PANEL: Color32 = Color32::from_rgb(28, 31, 32);
const SCREEN: Color32 = Color32::from_rgb(13, 17, 18);
const TEAL: Color32 = Color32::from_rgb(115, 209, 193);
const OCHRE: Color32 = Color32::from_rgb(237, 178, 112);
const LILAC: Color32 = Color32::from_rgb(183, 163, 226);

/// Vertical drag, arrow keys and the secondary-button MIDI menu share the same control.
fn rotary(ui: &mut Ui, label: &str, value: &mut f32, ink: Color32, midi: &str, bipolar: bool) {
    let min = if bipolar { -1.0 } else { 0.0 };
    let span = 1.0 - min;
    let size = ui.available_width().clamp(32.0, 46.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let response = response.midi_menu(ui, midi);
    if response.clicked() { response.request_focus(); }
    if bipolar && response.double_clicked_by(egui::PointerButton::Primary) { *value = 0.0; }
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = ui.input(|i| -i.pointer.delta().y / if i.modifiers.shift { 800.0 } else { 145.0 });
        *value = (*value + span * delta).clamp(min, 1.0);
    }
    if response.has_focus() {
        ui.input(|i| {
            let up = i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowRight);
            let down = i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowLeft);
            *value = (*value + (i32::from(up) - i32::from(down)) as f32 * 0.01).clamp(min, 1.0);
        });
    }
    let center = rect.center();
    let radius = size * 0.43;
    let position = |fraction: f32, r: f32| {
        let angle = (-0.75 + 1.5 * fraction) * PI;
        center + Vec2::new(angle.sin(), -angle.cos()) * r
    };
    let arc = |start: f32, end: f32| (0..=32).map(|n| {
        position(start + (end - start) * n as f32 / 32.0, radius)
    }).collect::<Vec<_>>();
    let fraction = (*value-min) / span;
    let painter = ui.painter();
    painter.circle_filled(center, radius - 3.0, Color32::from_rgb(40, 44, 45));
    painter.add(egui::Shape::line(arc(0.0, 1.0), Stroke::new(2.5, BORDER)));
    let zero = if bipolar { 0.5 } else { 0.0 };
    if bipolar {
        painter.line_segment([position(zero, radius-2.0),position(zero, radius+2.0)], Stroke::new(1.0, DIM));
    }
    painter.add(egui::Shape::line(arc(zero, fraction), Stroke::new(2.5, ink)));
    painter.line_segment([position(fraction, radius * 0.47), position(fraction, radius * 0.74)], Stroke::new(2.0, INK));
    if response.has_focus() {
        painter.circle_stroke(center, radius + 3.0, Stroke::new(1.0, ink));
    }
    let tip = if bipolar { "Double-click to reset to zero." } else { "Hold Shift for fine adjustment." };
    response.on_hover_text(format!("{label}: {:.0}%\nDrag or use arrow keys. {tip}", *value * 100.0));
}

fn dial(ui: &mut Ui, label: &str, value: &mut f32, ink: Color32, midi: &str) {
    ui.vertical_centered(|ui| {
        rotary(ui, label, value, ink, midi, false);
        midi_label(ui, RichText::new(label).size(11.0).color(INK), midi);
    });
}

fn target(row: usize, field: &str) -> String {
    format!("acidsynth{}.{field}", row + 1)
}

fn midi_label(ui: &mut Ui, text: RichText, id: &str) {
    ui.add(egui::Label::new(text).sense(Sense::click())).midi_menu(ui, id);
}

fn dials(ui: &mut Ui, row: usize, params: &mut [(&str, &str, &mut f32)], ink: Color32) {
    ui.columns(params.len(), |cols| {
        for (col, (field, label, value)) in cols.iter_mut().zip(params) {
            dial(col, label, value, ink, &target(row, field));
        }
    });
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

fn oscillator_graph(ui: &mut Ui, patch: &Patch, macros: [f32; 4]) {
    if patch.osc_mode >= 3 {
        let key = ui.make_persistent_id("oscillator-preview");
        let saved = ui.ctx().data(|d| d.get_temp::<(Patch, [f32; 4], [f32; 181])>(key));
        let cycle = match saved {
            Some((previous, knobs, trace)) if previous == *patch && knobs == macros => trace,
            _ => {
                let mut trace = [0.0; 181];
                orb_engine::studio::acid_synth::model_preview(patch, macros, &mut trace);
                ui.ctx().data_mut(|d| d.insert_temp(key, (*patch, macros, trace)));
                trace
            }
        };
        graph(ui, TEAL, |x| 0.5 + cycle[(x * 180.0).round() as usize] * 0.44);
        return;
    }
    // Only oscillator controls invalidate the classic preview. Mixer, filter and envelope
    // gestures repaint this panel continuously while leaving these partial sums identical.
    let shape = [patch.wave_a, patch.wave_b, patch.mix, f32::from(patch.transpose_b), f32::from(patch.osc_mode)];
    let key = ui.make_persistent_id("classic-oscillator-preview");
    let saved = ui.ctx().data(|d| d.get_temp::<([f32; 5], [f32; 181])>(key));
    let cycle = match saved {
        Some((previous, trace)) if previous == shape => trace,
        _ => {
            let ratio = 2.0_f32.powf(f32::from(patch.transpose_b) / 12.0);
            let trace = std::array::from_fn(|i| {
                let phase = 2.0 * i as f32 / 180.0;
                let a = morph(phase, patch.wave_a);
                let b = morph(phase * ratio, patch.wave_b);
                match patch.osc_mode {
                    1 => a + (1.6 * a * b - a) * patch.mix,
                    2 => morph(phase + 0.22 * b * patch.mix, patch.wave_a),
                    _ => a + (b - a) * patch.mix,
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(key, (shape, trace)));
            trace
        }
    };
    graph(ui, TEAL, |x| 0.5 + cycle[(x * 180.0).round() as usize] * 0.44);
}

/// The bright operators feed the output, the connecting arrows carry phase modulation.
fn operator_network(ui: &mut Ui, algorithm: u8) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 49.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, SCREEN);
    let (positions, edges, carriers): ([(f32, f32); 4], &[(usize, usize)], &[usize]) = match algorithm {
        1 => ([(0.36, 0.5), (0.12, 0.5), (0.88, 0.5), (0.64, 0.5)], &[(1,0),(3,2)], &[0,2]),
        2 => ([(0.87, 0.5), (0.11, 0.26), (0.38, 0.5), (0.11, 0.74)], &[(1,0),(2,0),(3,0)], &[0]),
        3 => ([(0.87, 0.5), (0.5, 0.24), (0.5, 0.76), (0.13, 0.5)], &[(3,1),(3,2),(1,0),(2,0)], &[0]),
        _ => ([(0.88, 0.5), (0.62, 0.5), (0.37, 0.5), (0.12, 0.5)], &[(3,2),(2,1),(1,0)], &[0]),
    };
    let nodes = positions.map(|(x,y)| rect.left_top() + Vec2::new(x * rect.width(), y * rect.height()));
    for &(from, to) in edges {
        let a = nodes[from] + Vec2::new(17.0, 0.0);
        let b = nodes[to] - Vec2::new(17.0, 0.0);
        let dir = (b-a).normalized();
        painter.line_segment([a,b], Stroke::new(1.2, TEAL));
        let tail = b - dir*5.0;
        let perpendicular = Vec2::new(-dir.y,dir.x)*2.5;
        painter.line_segment([tail+perpendicular,b], Stroke::new(1.2,TEAL));
        painter.line_segment([tail-perpendicular,b], Stroke::new(1.2,TEAL));
    }
    for (i, &center) in nodes.iter().enumerate() {
        let carrier = carriers.contains(&i);
        let bounds = egui::Rect::from_center_size(center, Vec2::new(33.0, 17.0));
        painter.rect_filled(bounds, 2.5, if carrier { TEAL } else { PANEL });
        painter.rect_stroke(bounds, 2.5, Stroke::new(1.0, TEAL), egui::StrokeKind::Inside);
        painter.text(center, egui::Align2::CENTER_CENTER, format!("OP{}", i+1),
            egui::FontId::monospace(9.0), if carrier { SCREEN } else { INK });
    }
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

fn modes(ui: &mut Ui, label: &str, midi: &str, value: &mut u8, names: &[&str]) {
    ui.horizontal_wrapped(|ui| {
        midi_label(ui, RichText::new(label).size(10.0).color(DIM), midi);
        for (i, &name) in names.iter().enumerate() {
            ui.selectable_value(value, i as u8, name).midi_menu(ui, midi);
        }
    });
}

fn matrix(ui: &mut Ui, row: usize, p: &mut Patch) {
    const SOURCES: [&str; 7] = ["Off", "LFO", "Amp env", "Timbre", "Movement", "Width", "Space"];
    let destinations = match p.osc_mode {
        3 => ["FM index", "Feedback", "Cutoff", "Balance", "Pitch", "Drive"],
        4 => ["Fold", "Sync", "Cutoff", "Wet mix", "Pitch", "Drive"],
        _ => ["Wave A", "Wave B", "Cutoff", "Balance", "Pitch", "Drive"],
    };
    let active = p.routes.iter().filter(|route| route.source != 0 && route.amount != 0.0).count();
    egui::CollapsingHeader::new(RichText::new(format!("Modulation matrix   {active} / 4")).monospace().color(LILAC))
        .id_salt(("acid-mod-matrix", row))
        .show(ui, |ui| {
            for (i, route) in p.routes.iter_mut().enumerate() {
                ui.push_id(("matrix-row", row, i), |ui| {
                    if i > 0 { ui.separator(); }
                    let route_id = format!("route{}", i + 1);
                    let amount_id = target(row, &format!("{route_id}.amount"));
                    let source_id = target(row, &format!("{route_id}.source"));
                    let dest_id = target(row, &format!("{route_id}.dest"));
                    let width = (ui.available_width() - 88.0).max(150.0);
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(Vec2::new(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_width(width);
                            modes(ui, &format!("FROM {}", i + 1), &source_id, &mut route.source, &SOURCES);
                            modes(ui, "TO", &dest_id, &mut route.dest, &destinations);
                        });
                        ui.allocate_ui_with_layout(Vec2::new(68.0, 0.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                            rotary(ui, "Route amount", &mut route.amount, LILAC, &amount_id, true);
                            let label = format!("{:+.0}%", route.amount * 100.0);
                            midi_label(ui, RichText::new(label).size(11.0).monospace().color(INK), &amount_id);
                        });
                    });
                });
            }
        });
}

fn preset_browser_id(row: usize) -> egui::Id {
    egui::Id::unique(("acid-sound-preset-browser", row))
}

/// An in-flow list keeps left-click browsing independent of the viewport's MIDI context popup.
fn preset_browser(ui: &mut Ui, row: usize, slot: &mut Slot) {
    let mut selected = None;
    let preset_id = target(row, "preset");
    Frame::new().fill(PANEL).inner_margin(Margin::same(8)).corner_radius(4.0).show(ui, |ui| {
        ui.columns(2, |columns| {
            for (column, (start, end, title)) in columns.iter_mut().zip([
                (0, EXPERIMENTAL_START, "Factory"),
                (EXPERIMENTAL_START, PRESETS.len(), "Experimental"),
            ]) {
                column.strong(title);
                egui::ScrollArea::vertical().id_salt(("preset-bank", row, start))
                    .max_height(165.0).show(column, |ui| {
                        for (i, (name, _)) in PRESETS.iter().enumerate().take(end).skip(start) {
                            if ui.selectable_label(usize::from(slot.preset) == i, *name)
                                .midi_menu(ui, &preset_id).clicked() {
                                selected = Some(i);
                            }
                        }
                    });
            }
        });
    });
    if let Some(index) = selected {
        apply_preset(slot, index);
    }
    if selected.is_some() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.ctx().data_mut(|d| d.insert_temp(preset_browser_id(row), false));
    }
}

fn patch_panel(ui: &mut Ui, slot: &mut Slot, row: usize) -> (bool, bool) {
    let mut audition = false;
    let before = slot.patch;
    let initial_macros = slot.macros;
    let initial_preset = slot.preset;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("AcidSynth").size(25.0).strong().color(INK));
        ui.label(RichText::new(["A", "B"][row]).monospace().color(TEAL));
        ui.add_space(12.0);
        let current = usize::from(slot.preset).min(PRESETS.len()-1);
        let suffix = if PRESETS[current].1 == slot.patch { "" } else { " *" };
        let preset_id = target(row, "preset");
        let picker_open = ui.ctx().data(|d| d.get_temp::<bool>(preset_browser_id(row))).unwrap_or(false);
        let preset_button = ui.add_sized([195.0, 24.0], egui::Button::new(format!("{}{suffix}", PRESETS[current].0))
            .selected(picker_open)).midi_menu(ui, &preset_id).on_hover_text("Choose a preset");
        if preset_button.clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(preset_browser_id(row), !picker_open));
        }
        audition = ui.button("Audition").midi_menu(ui, &target(row, "audition"))
            .on_hover_text("Play the selected chord for three seconds.").clicked();
    });
    if ui.ctx().data(|d| d.get_temp::<bool>(preset_browser_id(row))).unwrap_or(false) {
        preset_browser(ui, row, slot);
    }
    ui.add_space(5.0);
    let p = &mut slot.patch;
    ui.columns(2, |cols| {
        module(&mut cols[0], "OSCILLATORS", TEAL, |ui| {
            oscillator_graph(ui, p, slot.macros);
            modes(ui, "ENGINE", &target(row, "osc_mode"), &mut p.osc_mode, &["Blend", "Ring", "Phase mod", "FM4", "Sync Fold"]);
            match p.osc_mode {
                3 => {
                    modes(ui, "FM ROUTING", &target(row, "fm_algorithm"), &mut p.fm_algorithm, &["Cascade", "Pairs", "Fan", "Branch"]);
                    operator_network(ui, p.fm_algorithm);
                    ui.horizontal_wrapped(|ui| {
                        for (n, ratio) in p.fm_ratios.iter_mut().enumerate() {
                            let id = target(row, &format!("fm_ratio{}", n + 2));
                            let drag = crate::midi::drag_value(ui, ratio).range(0.25..=12.0).speed(0.025)
                                .fixed_decimals(2).prefix(format!("OP{} ×", n+2));
                            ui.add(drag).midi_menu(ui, &id);
                        }
                    });
                    dials(ui, row, &mut [
                        ("fm_index", "Index", &mut p.fm_index),
                        ("fm_feedback", "Feedback", &mut p.fm_feedback),
                        ("mix", "Balance", &mut p.mix),
                    ], TEAL);
                }
                4 => {
                    dials(ui, row, &mut [
                        ("fold", "Fold", &mut p.fold), ("sync", "Sync", &mut p.sync),
                        ("mix", "Wet mix", &mut p.mix),
                    ], TEAL);
                }
                _ => {
                    dials(ui, row, &mut [
                        ("wave_a", "Table A", &mut p.wave_a), ("wave_b", "Table B", &mut p.wave_b),
                        ("mix", "Balance", &mut p.mix), ("detune", "Detune", &mut p.detune),
                    ], TEAL);
                    ui.horizontal(|ui| {
                        let id = target(row,"transpose_b");
                        midi_label(ui, RichText::new("Osc B pitch").size(11.0).color(DIM), &id);
                        ui.add(crate::midi::drag_value(ui, &mut p.transpose_b).range(-24..=24).suffix(" st")).midi_menu(ui, &id);
                    });
                }
            }
        });
        module(&mut cols[1], "MULTIMODE FILTER", ACID, |ui| {
            filter_graph(ui, p);
            modes(ui, "MODE", &target(row, "filter_mode"), &mut p.filter_mode, &["LP", "BP", "HP", "Notch"]);
            dials(ui, row, &mut [("cutoff", "Cutoff", &mut p.cutoff), ("resonance", "Resonance", &mut p.resonance), ("filter_env", "Env depth", &mut p.filter_env)], ACID);
        });
    });
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        module(&mut cols[0], "AMPLITUDE ENVELOPE", OCHRE, |ui| {
            envelope_graph(ui, p);
            dials(ui, row, &mut [("attack", "Attack", &mut p.attack), ("decay", "Decay", &mut p.decay), ("sustain", "Sustain", &mut p.sustain), ("release", "Release", &mut p.release)], OCHRE);
        });
        module(&mut cols[1], "MODULATION", LILAC, |ui| {
            lfo_graph(ui, p);
            modes(ui, "WAVE", &target(row, "lfo_shape"), &mut p.lfo_shape, &["Sine", "Tri", "Ramp", "Square"]);
            modes(ui, "TO", &target(row, "lfo_target"), &mut p.lfo_target, &["Colour", "Filter", "Pitch", "Mix"]);
            dials(ui, row, &mut [("lfo_rate", "Rate", &mut p.lfo_rate), ("lfo_depth", "Depth", &mut p.lfo_depth)], LILAC);
        });
    });
    ui.add_space(4.0);
    ui.columns(2, |cols| {
        module(&mut cols[0], "CHARACTER", TEAL, |ui| {
            dials(ui, row, &mut [("sub", "Sub", &mut p.sub), ("drive", "Drive", &mut p.drive), ("chorus", "Chorus", &mut p.chorus)], TEAL);
        });
        module(&mut cols[1], "PERFORMANCE MACROS", ACID, |ui| {
            ui.columns(4, |cols| {
                for (i, (key, title)) in [("timbre","Timbre"),("movement","Movement"),("width","Width"),("space","Space")].into_iter().enumerate() {
                    let midi = format!("acidsynth{}.{key}", row+1);
                    dial(&mut cols[i], title, &mut slot.macros[i], ACID, &midi);
                }
            });
        });
    });
    matrix(ui, row, &mut slot.patch);
    (before != slot.patch || initial_macros != slot.macros || initial_preset != slot.preset, audition)
}

impl App {
    pub(super) fn acid_editor(&mut self, ui: &mut Ui, row: usize, id: egui::Id) {
        let mut open = true;
        let mut changed = false;
        let mut audition = false;
        let slot = &mut self.studio.settings.synth_slots[row];
        egui::Window::new(format!("AcidSynth {} / Sound editor", ["A","B"][row]))
            .open(&mut open).default_width(690.0).default_height(700.0).min_width(540.0).vscroll(true)
            .show(ui.ctx(), |ui| {
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(56, 74, 71);
                ui.visuals_mut().selection.stroke = Stroke::new(1.0, TEAL);
                (changed, audition) = patch_panel(ui, slot, row);
            });
        if audition {
            self.audition_acid_patch(row);
        }
        if changed {
            self.studio.changed();
            self.doc.modified = true;
        }
        if !open {
            ui.ctx().data_mut(|d| d.insert_temp(id, None::<usize>));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_shapes(shape: &egui::Shape, labels: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                labels.push((text.galley.text().into(), rect.center()));
            }
            egui::Shape::Vec(shapes) => {
                for child in shapes { text_shapes(child, labels); }
            }
            _ => {}
        }
    }

    fn panel_frame(ctx: &egui::Context, slot: &mut Slot, events: Vec<egui::Event>) -> (Vec<(String, Pos2)>, (bool, bool)) {
        let mut interaction = (false, false);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(780.0, 1300.0))),
                events,
                ..Default::default()
            },
            |ui| { interaction = patch_panel(ui, slot, 1); },
        );
        let mut labels = Vec::new();
        for clipped in &output.shapes { text_shapes(&clipped.shape, &mut labels); }
        output.textures_delta.clear();
        (labels, interaction)
    }

    fn frame(ctx: &egui::Context, slot: &mut Slot, events: Vec<egui::Event>) -> Vec<(String, Pos2)> {
        let (labels, interaction) = panel_frame(ctx, slot, events);
        assert_eq!(interaction, (false, false), "secondary click edited or auditioned a patch");
        labels
    }

    fn primary(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    fn click(ctx: &egui::Context, slot: &mut Slot, pos: Pos2) {
        panel_frame(ctx, slot, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] { panel_frame(ctx, slot, vec![primary(pos, pressed)]); }
    }

    fn position(labels: &[(String, Pos2)], text: &str) -> Pos2 {
        labels.iter().find(|(s,_)| s == text).unwrap_or_else(|| panic!("missing {text}")).1
    }

    #[test]
    fn primary_click_selects_factory_and_experimental_presets_in_edit_sound() {
        let ctx = egui::Context::default();
        let mut slot = Slot::default();
        for index in [1, EXPERIMENTAL_START] {
            let labels = panel_frame(&ctx, &mut slot, vec![]).0;
            let current = PRESETS[usize::from(slot.preset)].0;
            click(&ctx, &mut slot, position(&labels, current));
            let labels = panel_frame(&ctx, &mut slot, vec![]).0;
            assert!(labels.iter().any(|(s,_)| s == "Experimental"));
            click(&ctx, &mut slot, position(&labels, PRESETS[index].0));
            assert_eq!(slot.patch, PRESETS[index].1);
            assert_eq!(usize::from(slot.preset), index);
            let labels = panel_frame(&ctx, &mut slot, vec![]).0;
            assert!(!labels.iter().any(|(s,_)| s == "Experimental"));
        }
    }

    #[test]
    fn choosing_connections_and_dragging_depth_keeps_the_matrix_expanded() {
        let ctx = egui::Context::default();
        let mut slot = Slot::default();
        slot.patch = Patch::INIT;
        panel_frame(&ctx, &mut slot, vec![]);
        let labels = panel_frame(&ctx, &mut slot, vec![]).0;
        let header = labels.iter().find(|(s,_)| s.contains("Modulation matrix")).unwrap().1;
        click(&ctx, &mut slot, header);
        // Let the normal collapsing-header animation settle before measuring control positions.
        for _ in 0..24 { panel_frame(&ctx, &mut slot, vec![]); }
        let labels = panel_frame(&ctx, &mut slot, vec![]).0;
        // Route 1, as distinct from similarly named labels in the synthesis modules above.
        let source_y = position(&labels, "FROM 1").y;
        let select = |labels: &[(String, Pos2)], text: &str| -> Pos2 {
            labels.iter().filter(|(s,pt)| s == text && pt.y >= source_y - 2.0)
                .min_by(|a,b| a.1.y.total_cmp(&b.1.y)).unwrap().1
        };
        click(&ctx, &mut slot, select(&labels, "LFO"));
        let labels = panel_frame(&ctx, &mut slot, vec![]).0;
        click(&ctx, &mut slot, select(&labels, "Pitch"));
        assert_eq!(slot.patch.routes[0].source, 1);
        assert_eq!(slot.patch.routes[0].dest, 4);
        let labels = panel_frame(&ctx, &mut slot, vec![]).0;
        let knob = position(&labels, "+0%") - Vec2::new(0.0, 29.0);
        panel_frame(&ctx, &mut slot, vec![egui::Event::PointerMoved(knob)]);
        panel_frame(&ctx, &mut slot, vec![primary(knob,true)]);
        let drag = knob + Vec2::new(0.0,35.0);
        panel_frame(&ctx, &mut slot, vec![egui::Event::PointerMoved(drag)]);
        panel_frame(&ctx, &mut slot, vec![primary(drag,false)]);
        assert!(slot.patch.routes[0].amount < -0.05);
        for _ in 0..24 { panel_frame(&ctx, &mut slot, vec![]); }
        let labels = panel_frame(&ctx, &mut slot, vec![]).0;
        assert!(labels.iter().any(|(s,_)| s.contains("Modulation matrix") && s.contains("1 / 4")));
        assert!(labels.iter().any(|(s,_)| s == "FROM 4"));
    }

    #[test]
    fn sound_editor_right_click_targets_the_slot_without_changing_the_patch() {
        for (label, field, y_offset) in [
            ("Table A", "wave_a", -27.0),
            ("Ring", "osc_mode", 0.0),
            ("Audition", "audition", 0.0),
            (PRESETS[0].0, "preset", 0.0),
        ] {
            let ctx = egui::Context::default();
            let mut slot = Slot::default();
            let initial = slot.clone();
            frame(&ctx, &mut slot, vec![]);
            let labels = frame(&ctx, &mut slot, vec![]);
            let (_, center) = labels.iter().find(|(name, _)| name == label)
                .unwrap_or_else(|| panic!("missing {label}"));
            let pos = *center + Vec2::new(0.0, y_offset);
            frame(&ctx, &mut slot, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                frame(&ctx, &mut slot, vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                }]);
            }
            frame(&ctx, &mut slot, vec![]);
            let labels = frame(&ctx, &mut slot, vec![]);
            assert!(labels.iter().any(|(name, _)| name == "MIDI LEARN..."), "{label}");
            let expected = crate::midi::label(&target(1, field));
            assert!(labels.iter().any(|(name, _)| name == &expected), "{label} / {expected}");
            assert_eq!(slot, initial);
        }
    }
}
