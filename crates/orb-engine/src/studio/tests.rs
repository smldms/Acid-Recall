use super::*;
use crate::{
    frame::{Devices, Engine},
    tables::Tables,
};
use std::sync::Arc;

#[test]
fn both_tb_and_acidsynth_pairs_sum_independently_before_channel_inserts() {
    fn run(tb: [bool; 2], synth: [bool; 2]) -> [[f32; 64]; 2] {
        let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]; 64]).unwrap()));
        for row in 0..2 {
            let slot = &mut studio.settings.synth_slots[row];
            slot.tb_enabled = Some(tb[row]);
            slot.enabled = synth[row];
            slot.sequence_enabled = false;
            slot.patch.attack = 0.0;
            studio.live_note(row, acid_live::Event::On {
                key: 1, pitch: 48 + 7 * row as u8, velocity: 105, grid: 0,
            }, 0.0, 0.0);
        }
        let mut buses = [[0.0; 64]; 2];
        for _ in 0..20 {
            for (row, bus) in buses.iter_mut().enumerate() {
                // Stand-ins for the distinct native 303 signals entering the strip.
                bus.fill(if row == 0 { 0.125 } else { -0.0625 });
                studio.synth_channel(row, bus, 0.0, 0.01, false);
            }
        }
        for row in 0..2 {
            assert_eq!(studio.synth_gate(row, 0.0, false), synth[row]);
            assert_eq!(studio.synth_gate(row, 0.0, true), synth[row] || tb[row]);
        }
        buses
    }

    let off = [false; 2];
    let on = [true; 2];
    let silent = run(off, off);
    let native = run(on, off);
    let acid = run(off, on);
    let both = run(on, on);
    let crossed = run([true, false], [false, true]);
    assert_eq!(silent, [[0.0; 64]; 2]);
    assert_eq!(crossed, [native[0], acid[1]]);
    for row in 0..2 {
        assert!(acid[row].iter().any(|v| v.abs() > 0.001));
        for i in 0..64 {
            assert_eq!(both[row][i], native[row][i] + acid[row][i]);
        }
    }
}

#[test]
fn arming_a_silent_layer_does_not_widen_or_reverberate_its_tb() {
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]; 64]).unwrap()));
    let slot = &mut studio.settings.synth_slots[0];
    slot.enabled = true;
    slot.tb_enabled = Some(true);
    slot.sequence_enabled = false;
    slot.macros[2] = 1.0;
    slot.macros[3] = 1.0;
    for _ in 0..20 {
        studio.begin();
        let mut bus = std::array::from_fn(|i| i as f32 / 128.0);
        studio.synth_channel(0, &mut bus, 0.0, 0.01, false);
        for (j, &sample) in bus.iter().enumerate() {
            assert_eq!(studio.strip_sample(0, j, sample, 1.0, 1.0), (f64::from(sample), f64::from(sample)));
        }
        assert_eq!(studio.send, [[0.0; 64]; 2]);
    }
}

#[test]
fn sampler_sidechain_uses_each_lanes_own_machine_and_voice_with_global_off() {
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]; 64]).unwrap()));
    for (i, frame) in [[0.5, 0.0], [0.0, 0.5]].into_iter().enumerate() {
        studio.sampler.replace(
            i,
            Some(Box::new(audio_file::Wave {
                frames: vec![frame; 44100],
            })),
        );
        let lane = &mut studio.settings.lanes[i];
        lane.mode = sampler::Mode::Loop;
        lane.gain = 1.0;
        lane.sidechain = true;
        lane.sidechain_source = i as u8 + 1;
        lane.sidechain_instrument = if i == 0 { 2 } else { 1 };
        // The gater is intentionally unrelated to the sidechain source.
        lane.gater_source = 0;
        studio.sampler.audition(i);
    }
    let mut counts = [[0; 12]; 2];
    let render = |studio: &mut Studio, counts| {
        studio.modulation(Modulation {
            gates: [false; 2],
            drum_triggers: [[0; 12]; 2],
            drum_counts: counts,
            audible: [true; 2],
            config: crate::sidechain::Config::default(),
            ticks_per_frame: 0.01,
        });
        let mut output = [[0.0; 64]; 2];
        studio.sampler.process(
            &studio.settings.lanes,
            0.0,
            0.01,
            false,
            &mut output,
            &mut [[0.0; 64]; 2],
        );
        output.map(|channel| channel.iter().sum::<f64>())
    };
    for _ in 0..20 {
        render(&mut studio, counts);
    }
    // Wrong instrument must not trigger the TR-808 SD route.
    counts[0][1] += 1;
    let unchanged = render(&mut studio, counts);
    assert!((unchanged[0] - unchanged[1]).abs() < 1e-6);
    counts[0][2] += 1;
    let ducked = render(&mut studio, counts);
    assert!(
        ducked[0] < ducked[1] * 0.8,
        "TR-808 SD must duck only lane 1"
    );
    counts[1][1] += 1;
    render(&mut studio, counts);
    let both = render(&mut studio, counts);
    assert!(both[1] < unchanged[1] * 0.5, "TR-909 BD must duck lane 2");
}

#[test]
fn legacy_sampler_sidechain_routes_default_to_global() {
    let lane = sampler::Lane {
        sidechain: true,
        sidechain_source: 2,
        sidechain_instrument: 8,
        ..sampler::Lane::default()
    };
    let mut saved = serde_json::to_value(&lane).unwrap();
    let object = saved.as_object_mut().unwrap();
    object.remove("sidechain_source");
    object.remove("sidechain_instrument");
    let restored: sampler::Lane = serde_json::from_value(saved).unwrap();
    assert!(restored.sidechain);
    assert_eq!(restored.sidechain_source, 0);
    assert_eq!(restored.sidechain_instrument, 1);
    let mut clamped = lane;
    clamped.sidechain_source = 255;
    clamped.sidechain_instrument = 255;
    clamped.sanitize();
    assert_eq!(clamped.sidechain_source, 2);
    assert_eq!(clamped.sidechain_instrument, 11);
}

#[test]
fn studio_meters_capture_all_channel_and_master_samples_without_touching_audio() {
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]; 64]).unwrap()));
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    left[37] = -1.25;
    right[11] = 0.8;
    studio.channel(0, 37, -1.5, 0.0);
    studio.sampler_bus.dry[1][19] = 1.1;
    let before = [left, right];
    studio.meter_output(&left, &right);
    assert_eq!([left, right], before);
    let meters = studio.meter_readings();
    assert_eq!(meters[0].peak, 1.5);
    assert!(meters[0].clip);
    assert!(meters[5].clip);
    assert_eq!(meters[6].peak, 1.25);
    assert!(meters[6].clip);
    assert!(!meters[7].clip);
    assert_eq!(crate::frame::to_i16(left[37]), i16::MIN);
}

#[test]
fn actual_sampler_overload_reaches_the_preconversion_master_meter() {
    let mut engine = sampler_engine(
        SamplerBusSettings {
            gain: 2.0,
            ..SamplerBusSettings::default()
        },
        0.0,
    );
    let studio = engine.frame.studio.as_mut().unwrap();
    studio.sampler.replace(
        0,
        Some(Box::new(audio_file::Wave {
            frames: vec![[1.5; 2]; 44100],
        })),
    );
    studio.sampler.audition(0);
    let output = render_sampler(&mut engine, 12);
    assert!(output.contains(&i16::MAX));
    let meters = engine.frame.studio.as_ref().unwrap().meter_readings();
    assert!(meters[4].clip && meters[5].clip);
    assert!(meters[6].clip && meters[7].clip);
    assert!(meters[6].hold > 1.0);
}

fn sampler_engine(bus: SamplerBusSettings, lane_send: f64) -> Engine {
    let mut engine = Engine::new(Arc::new(Tables::new()), Devices::implemented(None));
    for channel in &mut engine.frame.devices.mixer.live.channels {
        channel.enabled = 0;
        channel.level = 0;
    }
    // Shared native effect settings are still authoritative for the sampler routes.
    engine.frame.devices.dist.set_param(0, 1);
    engine.frame.devices.dist.set_param(1, 110);
    engine.frame.devices.delay.set_param(0, 1);
    engine.frame.devices.delay.set_param(1, 1);
    engine.frame.devices.delay.set_param(3, 90);
    engine.frame.mixer_update();
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap()));
    studio.settings.sampler_bus = bus;
    studio.settings.predelay_ms = 0.0;
    studio.settings.lanes[0].send = lane_send;
    studio.settings.lanes[0].mode = sampler::Mode::Loop;
    studio.sampler.replace(
        0,
        Some(Box::new(audio_file::Wave {
            frames: (0..44100)
                .map(|j| {
                    let value =
                        (j as f64 * std::f64::consts::TAU * 220.0 / 44100.0).sin() as f32 * 0.2;
                    [value; 2]
                })
                .collect(),
        })),
    );
    studio.sampler.audition(0);
    engine.frame.studio = Some(Box::new(studio));
    engine
}

fn render_sampler(engine: &mut Engine, calls: usize) -> Vec<i16> {
    let mut result = Vec::new();
    let mut block = vec![0; 2048];
    for _ in 0..calls {
        engine.render(&mut block, 1024);
        result.extend_from_slice(&block);
    }
    result
}

#[test]
fn sampler_bus_native_dist_delay_and_space_routes_are_audible() {
    let baseline = render_sampler(&mut sampler_engine(SamplerBusSettings::default(), 0.0), 24);
    assert!(baseline.iter().any(|v| v.unsigned_abs() > 100));
    for bus in [
        SamplerBusSettings {
            dist: true,
            ..SamplerBusSettings::default()
        },
        SamplerBusSettings {
            delay: 1.0,
            ..SamplerBusSettings::default()
        },
        SamplerBusSettings {
            reverb_send: 1.0,
            ..SamplerBusSettings::default()
        },
    ] {
        let output = render_sampler(&mut sampler_engine(bus, 0.0), 24);
        assert!(
            output
                .iter()
                .zip(&baseline)
                .any(|(a, b)| a.abs_diff(*b) > 10),
            "{bus:?}"
        );
    }
    let lane_only = render_sampler(&mut sampler_engine(SamplerBusSettings::default(), 1.0), 24);
    assert!(
        lane_only
            .iter()
            .zip(&baseline)
            .any(|(a, b)| a.abs_diff(*b) > 10)
    );
}

#[test]
fn sampler_bus_gain_zero_silences_the_sampler_after_its_meter_detected_audio() {
    let mut engine = sampler_engine(SamplerBusSettings::default(), 0.0);
    assert!(
        render_sampler(&mut engine, 4)
            .iter()
            .any(|v| v.unsigned_abs() > 100)
    );
    assert!(
        engine
            .frame
            .studio
            .as_ref()
            .unwrap()
            .sampler_meter()
            .iter()
            .any(|&v| v > 0)
    );
    engine
        .frame
        .studio
        .as_mut()
        .unwrap()
        .settings
        .sampler_bus
        .gain = 0.0;
    render_sampler(&mut engine, 12);
    assert!(
        render_sampler(&mut engine, 1)
            .iter()
            .all(|v| v.unsigned_abs() <= 1)
    );
}

#[test]
fn an_empty_studio_is_sample_exact_with_the_legacy_engine() {
    let tables = Arc::new(Tables::new());
    let mut legacy = Engine::new(tables.clone(), Devices::implemented(None));
    let mut extended = Engine::new(tables, Devices::implemented(None));
    let mut song = rbs::Song::empty();
    song.glob.song_mode = 0;
    song.tb303[0].params[0] = 1;
    for value in &mut song.tb303[0].params[3..] {
        *value = 64;
    }
    song.tb303[0].patterns[0].length = 16;
    for (i, step) in song.tb303[0].patterns[0].steps.iter_mut().enumerate() {
        step.note = [0, 3, 7, 10][i % 4];
        step.rest = 0;
    }
    song.mixer.master = 80;
    song.mixer.channels[0].enabled = 1;
    song.mixer.channels[0].level = 80;
    let song = crate::load::convert(&song).unwrap();
    legacy.load(&song).unwrap();
    extended.load(&song).unwrap();
    extended.frame.studio = Some(Box::new(Studio::new(Box::new(
        Convolver::new(&[[0.1; 2]; 64]).unwrap(),
    ))));
    legacy.play();
    extended.play();
    let mut a = vec![0i16; 2048];
    let mut b = a.clone();
    let mut audible = false;
    for _ in 0..20 {
        legacy.render(&mut a, 1024);
        extended.render(&mut b, 1024);
        assert_eq!(a, b);
        audible |= a.iter().any(|x| x.unsigned_abs() > 10);
    }
    assert!(
        audible,
        "the bypass comparison must include an audible TB line"
    );
}

#[test]
fn queued_ir_requests_keep_latest_and_retire_every_root() {
    let make = || Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap());
    let mut studio = Studio::new(make());
    assert!(studio.replace_reverb(make()).is_none());
    assert!(studio.replace_reverb(make()).is_none());
    assert!(studio.replace_reverb(make()).is_some());
    studio.fade = 0;
    // Retirement can be delayed by a full garbage queue; a new request supersedes pending.
    assert!(studio.replace_reverb(make()).is_some());
    assert!(studio.retired().is_some());
    assert!(studio.pending.is_none());
    assert_eq!(studio.fade, 4410);
    studio.fade = 0;
    assert!(studio.retired().is_some());
    assert!(studio.previous.is_none());
}

#[test]
fn fading_states_reach_zero_instead_of_entering_subnormal_arithmetic() {
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap()));
    studio.smooth_sends = [1e-200; 4];
    studio.smooth_return = 1e-200;
    studio.smooth_delay = 1e-200;
    studio.high_state = [1e-200; 2];
    studio.low_state = [1e-200; 2];
    studio.settings.enabled = false;
    studio.settings.predelay_ms = 0.0;
    studio.begin();
    studio.channel(0, 0, 0.0, 0.0);
    studio.process(0.0, 0.01, false, &mut [0.0; 64], &mut [0.0; 64]);
    assert_eq!(studio.smooth_sends[0], 0.0);
    assert_eq!(studio.smooth_return, 0.0);
    assert_eq!(studio.smooth_delay, 0.0);
    assert_eq!(studio.high_state, [0.0; 2]);
    assert_eq!(studio.low_state, [0.0; 2]);
}

#[test]
fn release_long_running_sampler_and_silence_soak() {
    if cfg!(debug_assertions) {
        return;
    }
    let mut studio = Studio::new(Box::new(
        Convolver::new(&convolution::synthetic(1, 1.8)).unwrap(),
    ));
    for i in 0..8 {
        studio.sampler.waves[i] = Some(Box::new(audio_file::Wave {
            frames: vec![[0.1, -0.1]; 44100],
        }));
        studio.settings.lanes[i].steps.fill(100);
        studio.settings.lanes[i].send = 0.3;
    }
    let start = std::time::Instant::now();
    for block in 0..(120 * 44100 / 64) {
        studio.begin();
        let playing = block < 60 * 44100 / 64;
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        studio.process(block as f64 * 0.64, 0.01, playing, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    }
    eprintln!(
        "STUDIO_SOAK: 120 seconds audio / {:.3} seconds CPU",
        start.elapsed().as_secs_f64()
    );
}

#[test]
fn release_performance_with_six_second_ir_and_32_sampler_voices() {
    if cfg!(debug_assertions) {
        return;
    }
    let ir = convolution::synthetic(2, 6.0);
    let mut studio = Studio::new(Box::new(Convolver::new(&ir).unwrap()));
    studio.settings.sends = [0.3; 4];
    for (i, lane) in studio.settings.lanes.iter_mut().enumerate() {
        lane.gain = 0.1;
        lane.send = 0.2;
        studio.sampler.waves[i] = Some(Box::new(audio_file::Wave {
            frames: vec![[0.1; 2]; 100000],
        }));
    }
    for _ in 0..4 {
        for i in 0..8 {
            studio.sampler.audition(i);
        }
        studio.begin();
        studio.process(0.0, 0.01, false, &mut [0.0; 64], &mut [0.0; 64]);
    }
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    let start = std::time::Instant::now();
    for _ in 0..690 {
        studio.begin();
        for i in 0..64 {
            studio.channel(0, i, 0.02, -0.01);
        }
        studio.process(0.0, 0.01, false, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        left.fill(0.0);
        right.fill(0.0);
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!("STUDIO_PERFORMANCE: 1.001 s audio, {elapsed:.3} s processing / 6 s IR / 32 voices");
    assert!(
        elapsed < 4.0,
        "Studio render is unexpectedly slow: {elapsed:.3} s"
    );
}
