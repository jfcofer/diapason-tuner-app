//! `Mpm::analyze` never allocates: every window size the accuracy sweep uses, over tones, noise,
//! silence and malformed frames, with allocation trapping armed (`docs/AUDIO_ENGINE.md` §7,
//! `docs/TESTING.md` §2). Nor does the pre-processing chain in front of it: `DcBlock`,
//! `NoiseGate`, both `Decimator`s, `FrameAssembler`, `RangeSelector`, `OctaveGuard` and
//! `Mpm::peak_near`, and all of them streamed together through the shipped `chain::Front` and
//! `chain::Analysis` (`common::Tuner`), including the verifier frames of the ranges below the
//! selected one, the octave guard's jumps and the gate learning while open. Construction allocates, by design,
//! and runs outside the trap.
//!
//! `assert_no_alloc` aborts the process on a trapped allocation; it cannot panic, because
//! panicking allocates. So the canary that proves the trap is armed runs in a child process: this
//! same test binary, filtered to one ignored test, with an environment variable choosing whether
//! the analysed closure also allocates.

// The trapping allocator only exists in debug builds; release builds compile the check out.
#![cfg(debug_assertions)]

use std::process::{Command, ExitStatus, Output};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use diapason_dsp::filter::DcBlock;
use diapason_dsp::frame::FrameAssembler;
use diapason_dsp::gate::{GateConfig, NoiseGate};
use diapason_dsp::octave::{Octave, OctaveGuard};
use diapason_dsp::pitch::{Mpm, MpmConfig, PitchEstimate};
use diapason_dsp::range::{Observation, Range, RangeSelector, RangeTable, SUPPORTED_RATES};
use diapason_dsp::synth::{self, Noise, Pluck};

pub mod common;
use common::Tuner;
use diapason_dsp::chain::Selection;

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const RATE: u32 = 48_000;
/// From the shortest window of the accuracy sweep to the longest it uses (`tests/accuracy.rs`).
const WINDOWS: [usize; 7] = [128, 256, 512, 1_024, 2_048, 4_096, 8_192];
const CHILD_MODE: &str = "DIAPASON_DSP_NO_ALLOC_CHILD";

/// Frames that drive every path through `analyze`: a clear estimate, a rejected one (noise below the
/// clarity floor), silence, a non-finite sample, and a frame of the wrong length.
fn frames(window: usize) -> Vec<Vec<f32>> {
    let mut sine = vec![0.0_f32; window];
    synth::sine(&mut sine, 1_234.5, RATE, 0.5, 0.3);
    let mut saw = vec![0.0_f32; window];
    synth::sawtooth(&mut saw, 3.0 * f64::from(RATE) / 128.0, RATE, 0.5, 0.0);
    let mut pluck = vec![0.0_f32; window];
    synth::pluck(&mut pluck, 1_000.0, RATE, 0.8, &Pluck::default());
    let mut noise = vec![0.0_f32; window];
    Noise::new(11).fill(&mut noise, 0.5);
    let silence = vec![0.0_f32; window];
    let mut broken = sine.clone();
    broken[window / 3] = f32::NAN;
    let short = vec![0.1_f32; window - 1];
    vec![sine, saw, pluck, noise, silence, broken, short]
}

/// Analyse every frame `rounds` times per window with the trap armed around each call. `extra`
/// runs inside the trap after each analysis; the canary uses it to allocate.
fn analyse_armed(rounds: usize, extra: impl Fn()) -> usize {
    let mut estimates = 0;
    for window in WINDOWS {
        let mut mpm = Mpm::new(MpmConfig::with_window(window)).expect("valid config");
        let frames = frames(window);
        for _ in 0..rounds {
            for frame in &frames {
                let estimate = assert_no_alloc(|| {
                    let estimate = mpm.analyze(frame, RATE);
                    extra();
                    estimate
                });
                estimates += usize::from(estimate.is_some());
            }
        }
    }
    estimates
}

#[test]
fn analysis_never_allocates() {
    let estimates = analyse_armed(8, || {});
    // Sine, sawtooth and pluck give an estimate at every window; the other four frames do not.
    assert_eq!(estimates, WINDOWS.len() * 8 * 3);
}

/// Input for the chain: silence, then a tone that walks down through every range with no gaps (so
/// the selector switches, the octave guard holds jumps, and the verifiers decide octaves and catch
/// the octave above a weak fundamental), then 2.5 s of loud noise (so the selector scans, the
/// analysis reports the open gate unpitched, and the gate learns the room).
fn chain_input(rate: u32) -> Vec<f32> {
    let second = usize::try_from(rate).unwrap();
    let mut signal = vec![0.0_f32; 23 * second / 4];
    for (k, (hz, weak)) in [
        (1_000.0, false),
        (196.0, false),
        (82.41, true),
        (164.82, false),
        (41.2, true),
        (82.41, false),
    ]
    .into_iter()
    .enumerate()
    {
        let note = &mut signal[second / 4 + k * second / 2..][..second / 2];
        if weak {
            synth::partials(note, hz, rate, 0.5, 0.0, &synth::WEAK_FUNDAMENTAL);
        } else {
            synth::sawtooth(note, hz, rate, 0.5, 0.0);
        }
    }
    Noise::new(5).fill(&mut signal[13 * second / 4..], 0.1);
    signal
}

/// Stream `chain_input` through the chain at every supported rate, in blocks of awkward sizes,
/// with the trap armed around each block: home in Mid at both rates, and in the lowest range at one
/// rate and the highest at the other. `extra` runs inside the trap after
/// each block. Returns the frames analysed.
fn stream_armed(extra: impl Fn()) -> Streamed {
    let mut streamed = Streamed::default();
    for rate in SUPPORTED_RATES {
        let signal = chain_input(rate);
        let edge = if rate == 48_000 {
            Range::Low
        } else {
            Range::High
        };
        for home in [edge, Range::Mid] {
            let mut chain = Tuner::new(rate, Selection::Auto(home));
            let mut previous = None;
            for block in signal.chunks(173) {
                assert_no_alloc(|| {
                    chain.push(block, |frame| {
                        streamed.frames += 1;
                        streamed.ambiguous += usize::from(frame.octave == Octave::Ambiguous);
                        streamed.switches +=
                            usize::from(previous.is_some_and(|range| range != frame.range));
                        previous = Some(frame.range);
                    });
                    extra();
                });
            }
        }
    }
    streamed
}

/// What [`stream_armed`] drove: frames, frames flagged `ambiguous octave`, range switches.
#[derive(Debug, Default)]
struct Streamed {
    frames: usize,
    ambiguous: usize,
    switches: usize,
}

#[test]
fn the_chain_never_allocates() {
    let streamed = stream_armed(|| {});
    // The paths the trap must cover were taken: ambiguity (verifiers pending) and switches.
    assert!(
        streamed.frames > 1_000 && streamed.ambiguous > 10 && streamed.switches >= 8,
        "{streamed:?}"
    );
}

/// Each stage's per-call path on its own, with the trap armed, so a failure names the stage.
#[test]
fn no_stage_allocates() {
    let rate = 48_000;
    let signal = chain_input(rate);
    let table = RangeTable::new(rate).unwrap();
    let mut dc = DcBlock::new(rate).unwrap();
    let mut gate = NoiseGate::new(rate, GateConfig::default()).unwrap();
    let mut decimator = table.decimator().unwrap();
    let mut band_limit = table.band_limit().unwrap();
    let mut frames = FrameAssembler::new(480, 30).unwrap();
    let mut selector = RangeSelector::new(&table, Range::Low);
    let mut guard = OctaveGuard::new();
    let mut mpm = Mpm::new(MpmConfig::with_window(480)).unwrap();
    let mut window = [0.0_f32; 480];
    synth::partials(
        &mut window,
        82.41,
        12_000,
        0.5,
        0.0,
        &synth::WEAK_FUNDAMENTAL,
    );
    let mut block = [0.0_f32; 256];
    let mut outputs = 0;
    let mut position = 0;
    for chunk in signal.chunks(256) {
        position += 256;
        let block = &mut block[..chunk.len()];
        block.copy_from_slice(chunk);
        assert_no_alloc(|| dc.process(block));
        assert_no_alloc(|| gate.process(block));
        assert_no_alloc(|| band_limit.process(block, |_| outputs += 1));
        assert_no_alloc(|| {
            decimator.process(block, |y| {
                frames.push(&[y], |_, _| outputs += 1);
            });
        });
        assert_no_alloc(|| {
            for observation in [
                Observation::Pitch(330.0),
                Observation::Unclear,
                Observation::Pitch(f64::NAN),
                Observation::Silent,
            ] {
                selector.observe(observation, position);
            }
            selector.observe_verifier(Range::Low, Some(82.41), position);
            selector.observe_verifier(Range::Mid, Some(82.41), position);
            let _ = selector.octave(Range::High, 164.8);
            gate.set_unpitched(true);
        });
        assert_no_alloc(|| {
            if let Some(pick) = mpm.analyze(&window, 12_000) {
                let up = mpm.peak_near(&window, 12_000, pick.period_samples / 2.0);
                let down = mpm.peak_near(&window, 12_000, pick.period_samples * 2.0);
                for _ in 0..20 {
                    guard.judge(pick, up, down, 16);
                }
                let octave = PitchEstimate {
                    frequency_hz: 2.0 * pick.frequency_hz,
                    ..pick
                };
                guard.judge(octave, None, Some(pick), 16);
                guard.miss(16);
                guard.shown(Some(pick.frequency_hz), true, 16);
                let _ = guard.must_repeat(&octave);
                let _ = mpm.peak_within(&window, 12_000, pick.period_samples * 2.0, 4);
                guard.switched();
            }
        });
    }
    assert!(outputs > signal.len());
}

#[test]
#[ignore = "runs only as the child of the_trap_is_armed"]
fn canary_child() {
    match std::env::var(CHILD_MODE).as_deref() {
        Ok("clean") => {
            analyse_armed(1, || {});
        }
        Ok("allocate") => {
            analyse_armed(1, || {
                drop(std::hint::black_box(Vec::<f32>::with_capacity(16)))
            });
        }
        Ok("clean-chain") => {
            stream_armed(|| {});
        }
        Ok("allocate-chain") => {
            stream_armed(|| drop(std::hint::black_box(Vec::<f32>::with_capacity(16))));
        }
        _ => {} // Not launched by the canary: nothing to prove.
    }
}

fn run_child(mode: &str) -> Output {
    let exe = std::env::current_exe().expect("test binary path");
    Command::new(exe)
        .args(["canary_child", "--exact", "--ignored", "--test-threads=1"])
        .env(CHILD_MODE, mode)
        .output()
        .expect("spawn child")
}

/// SIGABRT on Linux and macOS, where `std::process::abort` lands.
#[cfg(unix)]
fn aborted(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(6)
}

#[cfg(not(unix))]
fn aborted(status: ExitStatus) -> bool {
    !status.success()
}

#[test]
fn the_trap_is_armed() {
    // Control: the same harness without the extra allocation runs to completion, so the child is
    // sound and a failure below can only be the trap firing. Once around `analyze`, once around
    // the whole chain.
    for (clean, allocate) in [("clean", "allocate"), ("clean-chain", "allocate-chain")] {
        let control = run_child(clean);
        let stdout = String::from_utf8_lossy(&control.stdout);
        assert!(control.status.success(), "{clean} child failed: {stdout}");
        assert!(
            stdout.contains("1 passed"),
            "{clean} child did not run the canary: {stdout}"
        );

        let dirty = run_child(allocate);
        assert!(
            aborted(dirty.status),
            "{allocate}: an allocation inside the trap was not caught (status {:?})",
            dirty.status
        );
    }
}
