//! `Mpm::analyze` never allocates: every window size the accuracy sweep uses, over tones, noise,
//! silence and malformed frames, with allocation trapping armed (`docs/AUDIO_ENGINE.md` §7,
//! `docs/TESTING.md` §2). Construction allocates, by design, and runs outside the trap.
//!
//! `assert_no_alloc` aborts the process on a trapped allocation; it cannot panic, because
//! panicking allocates. So the canary that proves the trap is armed runs in a child process: this
//! same test binary, filtered to one ignored test, with an environment variable choosing whether
//! the analysed closure also allocates.

// The trapping allocator only exists in debug builds; release builds compile the check out.
#![cfg(debug_assertions)]

use std::process::{Command, ExitStatus, Output};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use diapason_dsp::pitch::{Mpm, MpmConfig};
use diapason_dsp::synth::{self, Noise, Pluck};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const RATE: u32 = 48_000;
/// From the shortest window of the accuracy sweep to the longest of `docs/AUDIO_ENGINE.md` §2.
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
    // sound and a failure below can only be the trap firing.
    let clean = run_child("clean");
    let stdout = String::from_utf8_lossy(&clean.stdout);
    assert!(clean.status.success(), "clean child failed: {stdout}");
    assert!(
        stdout.contains("1 passed"),
        "clean child did not run the canary: {stdout}"
    );

    let dirty = run_child("allocate");
    assert!(
        aborted(dirty.status),
        "an allocation inside the trap was not caught (status {:?})",
        dirty.status
    );
}
