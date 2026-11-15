use std::sync::Arc;

use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};

use super::*;
use crate::link::Link;

fn cc(controller: u8, value: u8) -> Cc {
    Cc {
        channel: 1,
        controller,
        value,
    }
}

#[test]
fn a_precise_filter_changes_the_actual_learn_target_and_cancels_stale_learning() {
    let mut midi = Midi {
        filter: "tr909.ch_level".into(),
        ..Midi::default()
    };
    midi.begin_learn();
    assert_eq!(midi.learning.as_deref(), Some("tr909.ch_level"));
    for filter in ["tr909.hh_level", "  HH level  ", "TR-909 / OH level"] {
        midi.filter = filter.into();
        let candidates = midi.target_candidates();
        assert_eq!(candidates, vec![("tr909.hh_level".into(), Kind::Knob)]);
        assert_eq!(midi.selected, "tr909.hh_level");
        midi.begin_learn();
        assert_eq!(midi.learning.as_deref(), Some("tr909.hh_level"));
    }
    midi.filter = "tr909.ch_level".into();
    midi.target_candidates();
    assert!(midi.learning.is_none());
    midi.begin_learn();
    assert_eq!(midi.learning.as_deref(), Some("tr909.ch_level"));
}

#[test]
fn an_empty_or_ambiguous_result_cannot_learn_a_hidden_previous_target() {
    let mut midi = Midi {
        filter: "tr909.ch_level".into(),
        ..Midi::default()
    };
    midi.begin_learn();
    midi.filter = "no matching parameter".into();
    midi.begin_learn();
    assert!(midi.selected.is_empty() && midi.learning.is_none());
    midi.filter = "level".into();
    midi.begin_learn();
    assert!(midi.selected.is_empty() && midi.learning.is_none());
}

#[test]
fn the_rendered_midi_window_reconciles_a_filter_with_the_previous_selection() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    app.midi.open = true;
    app.midi.selected = "tr909.ch_level".into();
    app.midi.filter = "tr909.hh_level".into();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 900.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    assert_eq!(app.midi.selected, "tr909.hh_level");
    app.midi.begin_learn();
    assert_eq!(app.midi.learning.as_deref(), Some("tr909.hh_level"));
}

fn calls(audio: &mut Audio) {
    let mut out = vec![0; 2 * CALL_FRAMES];
    audio.render_i16(&mut out);
    audio.render_i16(&mut out);
}

#[test]
fn learning_ch_then_oh_keeps_two_assignments_and_controls_the_correct_engine_levels() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    app.settings.midi.pickup = false;
    app.settings.midi.minimum_one = true;
    for (target, controller) in [("tr909.ch_level", 74), ("tr909.hh_level", 75)] {
        app.midi.filter = target.into();
        app.midi.begin_learn();
        assert_eq!(app.midi.learning.as_deref(), Some(target));
        assert!(app.midi.handle_cc(
            cc(controller, 50),
            &mut app.settings.midi,
            &mut app.panel,
            0.0
        ));
    }
    assert_eq!(app.settings.midi.bindings.len(), 2);
    assert_eq!(app.settings.midi.bindings[0].target, "tr909.ch_level");
    assert_eq!(app.settings.midi.bindings[1].target, "tr909.hh_level");
    let ctx = egui::Context::default();
    app.midi
        .handle_cc(cc(74, 1), &mut app.settings.midi, &mut app.panel, 0.0);
    app.sync_effects(&ctx);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.midi
        .handle_cc(cc(75, 127), &mut app.settings.midi, &mut app.panel, 0.0);
    app.sync_effects(&ctx);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert_eq!(app.panel.value("tr909.ch_level"), Some(0));
    assert_eq!(app.panel.value("tr909.hh_level"), Some(127));
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        0
    );
}

#[test]
fn reusing_one_cc_reports_which_assignment_was_replaced() {
    let mut midi = Midi::default();
    let mut config = Config::default();
    let mut panel = Panel::new();
    for target in ["tr909.ch_level", "tr909.hh_level"] {
        midi.filter = target.into();
        midi.begin_learn();
        assert!(midi.handle_cc(cc(74, 64), &mut config, &mut panel, 0.0));
    }
    assert_eq!(config.bindings.len(), 1);
    assert_eq!(config.bindings[0].target, "tr909.hh_level");
    let notice = midi.notice.unwrap();
    assert!(notice.contains("OH level (Open Hat)"));
    assert!(notice.contains("Replaced TR-909 / CH level (Closed Hat)"));
}
