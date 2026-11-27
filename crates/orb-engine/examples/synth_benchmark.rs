//! Native engine stress test. Run with cargo run --locked --release -p orb-engine --example synth_benchmark.
//! Wall times include scheduler interruptions. The sample fingerprint excludes all timing data.
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Instant;

use orb_engine::frame::{BLOCK, Devices, Engine};
use orb_engine::studio::Studio;
use orb_engine::studio::acid_live::Event;
use orb_engine::studio::acid_synth::Patch;
use orb_engine::studio::convolution::{Convolver, synthetic};
use orb_engine::tables::Tables;

const BLOCKS: usize = 3072;
const RATE: f64 = 44_100.0;
const KEYS: [u8; 12] = [43, 47, 50, 53, 57, 60, 64, 67, 70, 74, 77, 81];

fn measure(tables: Arc<Tables>, name: &str, modes: [u8; 2], room: bool) -> Result<(), Box<dyn Error>> {
    let mut studio = Studio::new(Box::new(Convolver::new(&synthetic(1, 1.25))?));
    studio.settings.enabled = room;
    for (i, mode) in modes.into_iter().enumerate() {
        let slot = &mut studio.settings.synth_slots[i];
        slot.enabled = true;
        slot.sequence_enabled = false;
        slot.patch = Patch {
            osc_mode: mode, fm_algorithm: 3, fm_ratios: [1.75, 2.63, 4.17],
            fm_index: 0.71, fm_feedback: 0.63, sync: 0.68, fold: 0.91,
            chorus: 0.62, drive: 0.51, sub: 0.37, mix: 0.69,
            attack: 0.0, cutoff: 0.68, ..Patch::INIT
        };
        for pitch in KEYS {
            studio.live_note(i, Event::On {
                key: u16::from(pitch), pitch, velocity: 81, grid: 0,
            }, 0.0, 0.0);
        }
    }
    let mut engine = Engine::new(tables, Devices::implemented(None));
    engine.frame.studio = Some(Box::new(studio));
    engine.mixer_global(0, 100);
    for row in 0..4 {
        engine.mixer_channel(row, 0, u8::from(row < 2));
        engine.mixer_channel(row, 1, 75);
        engine.mixer_channel(row, 2, 64);
    }
    engine.play();
    let (mut left, mut right) = ([0.0; BLOCK], [0.0; BLOCK]);
    let mut durations = Vec::with_capacity(BLOCKS);
    let mut fingerprint = 0xcbf29ce484222325_u64;
    let mut energy = 0.0_f64;

    // Warm both voice banks and the full 1.25-second room before measuring.
    for i in 0..1024 + BLOCKS {
        let start = Instant::now();
        engine.frame.block(&mut left, &mut right).map_err(|e| io::Error::other(format!("{e:?}")))?;
        let ns = start.elapsed().as_nanos() as u64;
        if i < 1024 { continue; }
        durations.push(ns);
        for sample in left.iter().chain(&right) {
            fingerprint = (fingerprint ^ u64::from(sample.to_bits())).wrapping_mul(0x100000001b3);
            energy += f64::from(*sample).powi(2);
        }
    }
    let rms = (energy / (BLOCKS * BLOCK * 2) as f64).sqrt();
    if !rms.is_finite() || rms < 0.001 { return Err("invalid or silent benchmark render".into()); }
    durations.sort_unstable();
    let mean = durations.iter().sum::<u64>() as f64 / BLOCKS as f64 / 1000.0;
    let limit = 1e9 * BLOCK as f64 / RATE;
    let late = durations.iter().filter(|&&ns| ns as f64 >= limit).count();
    let us = |q: f64| durations[((BLOCKS - 1) as f64 * q).round() as usize] as f64 / 1000.0;
    println!("{name}: blocks={BLOCKS} mean_us={mean:.2} p50_us={:.2} p95_us={:.2} p99_us={:.2} max_us={:.2} over_budget={late} rms={rms:.6} hash={fingerprint:016x}",
        us(0.5), us(0.95), us(0.99), us(1.0));
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("44.1 kHz stereo, {BLOCK} frames, 12 held voices per synth, block budget {:.2} us", 1e6 * BLOCK as f64 / RATE);
    let tables = Arc::new(Tables::new());
    measure(Arc::clone(&tables), "FM4+FM4 dry", [3, 3], false)?;
    measure(Arc::clone(&tables), "Fold+Fold dry", [4, 4], false)?;
    measure(tables, "FM4+Fold 1.25s room", [3, 4], true)
}
