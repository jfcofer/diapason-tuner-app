//! Accuracy of the MPM detector on synthetic fixtures (`docs/AUDIO_ENGINE.md` §1 and §3,
//! `docs/TESTING.md` §2).
//!
//! Every case renders its frame from [`diapason_dsp::synth`], so the true frequency is known
//! exactly. The sweep runs from 30 to 1400 Hz at 24 points per octave and 48 kHz, at several
//! starting phases, because the window-edge terms of the NSDF depend on where in the cycle the
//! frame starts.
//!
//! Two window rules:
//!
//! - [`analysis_window`], the rule `T-003a` took from `docs/AUDIO_ENGINE.md` §2, sizes the window
//!   to the *range* the tone is in. The detector's budget is gated with it, on bare frames at
//!   48 kHz. The windows, ranges and filters the tuner actually uses are gated through the whole
//!   chain in `tests/chain.rs` (`docs/adr/0028`).
//! - [`stress_window`] holds only three periods of the tone itself, far shorter than the engine
//!   ever uses. Its results are recorded under regression ceilings, not gated against the budget.
//!
//! `cargo test -p diapason_dsp --test accuracy -- --nocapture` prints a summary per signal.

use std::collections::BTreeMap;
use std::f64::consts::TAU;

use diapason_dsp::convert::index_to_f64;
use diapason_dsp::pitch::{Mpm, MpmConfig, PitchEstimate};
use diapason_dsp::synth::{self, Noise, Pluck};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

pub mod common;
use common::report;

const RATE: u32 = 48_000;
/// `docs/AUDIO_ENGINE.md` §1: synthetic steady tones, 30–1400 Hz.
const BUDGET_CENTS: f64 = 0.5;
const LOWEST_HZ: f64 = 30.0;
const HIGHEST_HZ: f64 = 1_400.0;
const POINTS_PER_OCTAVE: u32 = 24;
/// Eight starting phases, evenly spaced over the cycle.
const PHASES: [f64; 8] = [
    0.0,
    TAU / 8.0,
    2.0 * TAU / 8.0,
    3.0 * TAU / 8.0,
    4.0 * TAU / 8.0,
    5.0 * TAU / 8.0,
    6.0 * TAU / 8.0,
    7.0 * TAU / 8.0,
];

type Generator = fn(&mut [f32], f64, u32, f64, f64);

/// Where proptest keeps the cases that failed, beside this file.
const REGRESSIONS: &str = "proptest-regressions";

/// `LOWEST_HZ · 2^(i/24)` up to `HIGHEST_HZ`, which is always included.
fn sweep() -> Vec<f64> {
    (0..)
        .map(|i| LOWEST_HZ * 2_f64.powf(f64::from(i) / f64::from(POINTS_PER_OCTAVE)))
        .take_while(|&hz| hz < HIGHEST_HZ)
        .chain(std::iter::once(HIGHEST_HZ))
        .collect()
}

/// The smallest power of two holding three periods of `hz`.
fn three_periods(hz: f64) -> usize {
    let mut window = 16;
    while index_to_f64(window) * hz < 3.0 * f64::from(RATE) {
        window *= 2;
    }
    window
}

/// The analysis window for a tone at `hz`, by the rule `T-003a` took from `docs/AUDIO_ENGINE.md`
/// §2: three periods of the *lowest* frequency of the range the tone falls in, rounded up to a
/// power of two, so every tone in a range gets the same window and a high one gets many periods.
/// The ranges are the ones §2 had then: below 60 Hz, 60–160 Hz, and 160 Hz and up, all at 48 kHz:
/// 8192 for 30–60 Hz (floor 30 Hz, the sweep's lowest), 4096 for 60–160 Hz, 1024 from 160 Hz.
/// `docs/adr/0028` has since replaced §2's table; this stays as the detector's own test.
fn analysis_window(hz: f64) -> usize {
    let floor = if hz >= 160.0 {
        160.0
    } else if hz >= 60.0 {
        60.0
    } else {
        LOWEST_HZ
    };
    three_periods(floor)
}

/// The stress rule: the smallest power-of-two window holding three periods of the tone itself.
fn stress_window(hz: f64) -> usize {
    three_periods(hz)
}

fn cents(measured_hz: f64, true_hz: f64) -> f64 {
    1_200.0 * (measured_hz / true_hz).log2()
}

/// One detector per window length, built once: construction is the expensive, allocating part.
struct Detectors(BTreeMap<usize, Mpm>);

impl Detectors {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn analyze(&mut self, frame: &[f32]) -> Option<PitchEstimate> {
        self.0
            .entry(frame.len())
            .or_insert_with(|| Mpm::new(MpmConfig::with_window(frame.len())).unwrap())
            .analyze(frame, RATE)
    }
}

/// One frequency at one starting phase, and how the detector did.
#[derive(Debug)]
struct Case {
    hz: f64,
    phase: f64,
    window: usize,
    cents: f64,
    clarity: f32,
}

/// Runs the sweep for one signal; returns every case, worst first.
fn run_sweep(generate: Generator, window_for: fn(f64) -> usize) -> Vec<Case> {
    let mut detectors = Detectors::new();
    let mut cases: Vec<Case> = sweep()
        .into_iter()
        .flat_map(|hz| PHASES.map(|phase| (hz, phase)))
        .map(|(hz, phase)| {
            let window = window_for(hz);
            let mut frame = vec![0.0_f32; window];
            generate(&mut frame, hz, RATE, 0.5, phase);
            let (cents, clarity) = match detectors.analyze(&frame) {
                Some(estimate) => (cents(estimate.frequency_hz, hz), estimate.clarity),
                None => (f64::INFINITY, 0.0),
            };
            Case {
                hz,
                phase,
                window,
                cents,
                clarity,
            }
        })
        .collect();
    cases.sort_by(|a, b| b.cents.abs().total_cmp(&a.cents.abs()));
    cases
}

/// The worst case, and the worst |cents| per octave band, for the `--nocapture` summary.
fn summary(name: &str, cases: &[Case]) {
    let mut bands: BTreeMap<i32, f64> = BTreeMap::new();
    for case in cases {
        let octave = (0..)
            .find(|&octave| case.hz < LOWEST_HZ * 2_f64.powi(octave + 1))
            .unwrap();
        let band = bands.entry(octave).or_default();
        *band = band.max(case.cents.abs());
    }
    let worst = &cases[0];
    let over = cases
        .iter()
        .filter(|c| c.cents.abs() > BUDGET_CENTS)
        .count();
    report(format_args!(
        "{name}: worst {:.4} cents at {:.2} Hz (window {}, phase {:.2}, clarity {:.4}); \
         over the budget in {over} of {} cases",
        worst.cents.abs(),
        worst.hz,
        worst.window,
        worst.phase,
        worst.clarity,
        cases.len(),
    ));
    for (octave, max) in bands {
        let low = LOWEST_HZ * 2_f64.powi(octave);
        report(format_args!(
            "  {low:7.1}–{:7.1} Hz: max |error| {max:.4} cents",
            (low * 2.0).min(HIGHEST_HZ)
        ));
    }
}

/// Fails with the worst case named if it is beyond `limit` cents.
fn assert_within(name: &str, cases: &[Case], limit: f64) {
    let worst = &cases[0];
    assert!(
        worst.cents.abs() <= limit,
        "{name} at {:.3} Hz (window {}, phase {:.3}): {:+.4} cents, clarity {:.4}; limit {limit}",
        worst.hz,
        worst.window,
        worst.phase,
        worst.cents,
        worst.clarity,
    );
}

#[test]
fn the_sweep_and_the_window_rules_are_as_documented() {
    let points = sweep();
    assert_eq!(points.first().copied(), Some(LOWEST_HZ));
    assert_eq!(points.last().copied(), Some(HIGHEST_HZ));
    // 24 · log2(1400/30) = 133.04, so i = 0..=133 lie below 1400 Hz: 134 points, then 1400 Hz.
    assert_eq!(points.len(), 135);
    let analysis = [30.0, 59.99, 60.0, 159.99, 160.0, 1_400.0].map(analysis_window);
    assert_eq!(analysis, [8_192, 8_192, 4_096, 4_096, 1_024, 1_024]);
    assert_eq!((stress_window(30.0), stress_window(1_400.0)), (8_192, 128));
}

/// The budget, gated: `docs/AUDIO_ENGINE.md` §1 with `T-003a`'s range windows.
///
/// A dense probe in `T-003a` (20 001 log-spaced frequencies over 30–1400 Hz × 32 phases) put the
/// worst case at −0.065 cents (1381.6 Hz, window 1024), a margin of more than 7× on this grid and
/// on the property test below. With `T-003b`'s common-overlap step 5 (`pitch::common_overlap`)
/// this grid's worst is 0.033 cents.
#[test]
fn a_sine_is_within_half_a_cent_across_the_sweep() {
    let cases = run_sweep(synth::sine, analysis_window);
    summary("sine, range windows", &cases);
    assert_within("sine", &cases, BUDGET_CENTS);
}

/// **Not the budget.** Three periods of the tone is shorter than any window the engine uses, and
/// there the NSDF's window-edge terms move the peak: the dense probe found −0.507 cents at
/// 1299.17 Hz (window 128), just past the budget. Recorded under a ceiling of 1 cent, the budget's
/// "fails at" (`docs/AUDIO_ENGINE.md` §1), which catches a broken detector, not that edge effect.
/// That probe predates `T-003b`'s common-overlap step 5, which removes most of the edge effect:
/// this grid's worst is now 0.104 cents.
#[test]
fn a_sine_in_three_period_windows_stays_under_its_ceiling() {
    let cases = run_sweep(synth::sine, stress_window);
    summary("sine, three-period windows", &cases);
    assert_within("sine (three periods)", &cases, 1.0);
}

/// **Not the budget.** A sawtooth band-limited to Nyquist misses the ±0.5 cent budget even with
/// `T-003a`'s range windows. The chain's band-limit meets it (`tests/chain.rs`, `T-003b`).
///
/// Parabolic interpolation assumes the NSDF peak is a parabola over ±1 lag. A sawtooth's peak is
/// `Σ wₕ cos(hω(τ − T))` with `wₕ ∝ 1/h²`, so every harmonic up to Nyquist adds equally to its
/// curvature: the peak is as sharp as the lag grid, and the vertex lands up to 0.053 lag off,
/// whatever the window. In cents that grows with frequency: the dense probe (5001 frequencies ×
/// 16 phases) found −2.86 cents at 1384 Hz (window 1024). The ceiling, 4 cents, catches a broken
/// detector (an octave is 1200 cents, one lag at 1400 Hz is about 50), not that bias. The
/// common-overlap step 5 of `T-003b` leaves the shape bias: this grid's worst is 2.77 cents.
#[test]
fn a_sawtooths_error_stays_under_its_ceiling() {
    let cases = run_sweep(synth::sawtooth, analysis_window);
    summary("sawtooth, range windows", &cases);
    assert_within("sawtooth", &cases, 4.0);
}

/// **Not the budget.** With three periods of the tone, the shape bias above adds to window-edge
/// distortion: harmonics near Nyquist alias in the NSDF's sums (`2hω ≈ 2π`), which distorts the
/// samples themselves, so that even interpolating with the exact peak shape leaves 2.2 cents. The
/// dense probe found −6.86 cents at 1127 Hz (window 128); the ceiling is 8. With `T-003b`'s
/// common-overlap step 5, this grid's worst is 3.90 cents.
#[test]
fn a_sawtooth_in_three_period_windows_stays_under_its_ceiling() {
    let cases = run_sweep(synth::sawtooth, stress_window);
    summary("sawtooth, three-period windows", &cases);
    assert_within("sawtooth (three periods)", &cases, 8.0);
}

/// Recorded, not gated: the budget for inharmonic strings belongs to `T-003c` and `T-003e`. The
/// reference is the first partial, `f₀·√(1+B)`, which is where the string's fundamental actually
/// vibrates. The frame starts 100 ms after the pluck.
#[test]
fn an_inharmonic_pluck_is_recorded() {
    let params = Pluck::default();
    let mut detectors = Detectors::new();
    report(format_args!(
        "pluck (B = {}, decay {} s, ≤ {} partials), error against f₁ = f₀·√(1+B):",
        params.inharmonicity, params.decay_seconds, params.max_partials
    ));
    // Guitar in standard tuning, then its 12th-fret octave of the top string.
    for f0 in [82.41, 110.0, 146.83, 196.0, 246.94, 329.63, 659.26] {
        let f1 = synth::partial_frequency(f0, 1, params.inharmonicity);
        let window = analysis_window(f1);
        let offset = 4_800;
        let mut signal = vec![0.0_f32; offset + window];
        for seed in 1..=4 {
            synth::pluck(&mut signal, f0, RATE, 0.8, &Pluck { seed, ..params });
            match detectors.analyze(&signal[offset..]) {
                Some(estimate) => report(format_args!(
                    "  f₀ {f0:7.2} Hz, seed {seed}, window {window:5}: {:+8.4} cents, \
                     clarity {:.4}",
                    cents(estimate.frequency_hz, f1),
                    estimate.clarity
                )),
                None => report(format_args!(
                    "  f₀ {f0:7.2} Hz, seed {seed}, window {window:5}: no estimate"
                )),
            }
        }
    }
}

#[test]
fn noise_alone_has_no_pitch() {
    let mut worst: f32 = 0.0;
    for window in [128, 256, 512, 1_024, 2_048, 4_096, 8_192] {
        let mut mpm = Mpm::new(MpmConfig::with_window(window)).unwrap();
        let mut frame = vec![0.0_f32; window];
        for seed in 0..32 {
            Noise::new(seed).fill(&mut frame, 0.5);
            if let Some(estimate) = mpm.analyze(&frame, RATE) {
                panic!(
                    "noise (window {window}, seed {seed}) gave {:.2} Hz at clarity {:.4}",
                    estimate.frequency_hz, estimate.clarity
                );
            }
            // With the floor at zero, the same frame reports the clarity it would have had.
            let mut open = Mpm::new(MpmConfig {
                clarity_floor: 0.0,
                ..MpmConfig::with_window(window)
            })
            .unwrap();
            if let Some(estimate) = open.analyze(&frame, RATE) {
                worst = worst.max(estimate.clarity);
            }
        }
    }
    report(format_args!(
        "noise: highest clarity over 7 windows × 32 seeds: {worst:.4}"
    ));
}

/// At 20 dB SNR the NSDF peak is near `P_s / (P_s + P_n) = 1/1.01 ≈ 0.99`: noise is uncorrelated
/// at the period lag, but adds to the energy term `m`.
#[test]
fn a_sine_at_20_db_snr_is_clear() {
    let mut detectors = Detectors::new();
    let mut lowest = (f32::INFINITY, 0.0, 0.0);
    for (i, hz) in sweep().into_iter().enumerate().step_by(4) {
        let window = analysis_window(hz);
        let mut frame = vec![0.0_f32; window];
        synth::sine(&mut frame, hz, RATE, 0.5, 1.0);
        let mut noise = vec![0.0_f32; window];
        Noise::new(u64::try_from(i).unwrap()).fill(&mut noise, 1.0);
        synth::mix_at_snr(&mut frame, &noise, 20.0);
        let estimate = detectors
            .analyze(&frame)
            .unwrap_or_else(|| panic!("sine at {hz:.2} Hz, 20 dB SNR: no estimate"));
        assert!(
            estimate.clarity > 0.9,
            "sine at {hz:.2} Hz, 20 dB SNR: clarity {:.4}, {:+.4} cents",
            estimate.clarity,
            cents(estimate.frequency_hz, hz)
        );
        if estimate.clarity < lowest.0 {
            lowest = (estimate.clarity, hz, cents(estimate.frequency_hz, hz));
        }
    }
    report(format_args!(
        "sine at 20 dB SNR: lowest clarity {:.4} at {:.2} Hz ({:+.4} cents)",
        lowest.0, lowest.1, lowest.2
    ));
}

proptest! {
    // An integration test has no lib.rs for proptest's default location to find; keep failures
    // beside this file, which is where the default falls back to anyway, minus the warning.
    #![proptest_config(ProptestConfig {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(REGRESSIONS))),
        ..ProptestConfig::default()
    })]

    /// Within budget across the range and from −40 to 0 dBFS, and independent of the gain, with
    /// `T-003a`'s range windows. Over 5000 random cases in `T-003a` the worst error was 0.064 cents, so the
    /// budget is not met by luck of the seed.
    ///
    /// `n = 2r/m` is homogeneous of degree zero in the signal, so in exact arithmetic the gain
    /// cancels. What remains is rounding: the frame is rounded to `f32` (relative error 2⁻²⁴ per
    /// sample whatever the gain, but in a different pattern) and the FFT runs in `f64`. Over the same
    /// 5000 cases that moved the estimate by at most 5.6·10⁻⁷ cents, so 10⁻⁴ cents leaves a factor
    /// of more than 100 for other platforms' `sin` and FFT kernels, while a detector with any
    /// gain-dependent step (an absolute threshold, say) would miss it by far.
    #[test]
    fn a_sine_is_within_budget_at_any_gain_and_does_not_depend_on_it(
        octaves in 0.0..(HIGHEST_HZ / LOWEST_HZ).log2(),
        gain_db in -40.0_f64..0.0,
        phase in 0.0..TAU,
    ) {
        let hz = LOWEST_HZ * 2_f64.powf(octaves);
        let window = analysis_window(hz);
        let mut mpm = Mpm::new(MpmConfig::with_window(window)).unwrap();
        let mut frame = vec![0.0_f32; window];

        synth::sine(&mut frame, hz, RATE, 10_f64.powf(gain_db / 20.0), phase);
        let quiet = mpm.analyze(&frame, RATE).expect("a clean sine has a pitch");
        synth::sine(&mut frame, hz, RATE, 1.0, phase);
        let full = mpm.analyze(&frame, RATE).expect("a clean sine has a pitch");

        let error = cents(quiet.frequency_hz, hz);
        assert!(
            error.abs() <= BUDGET_CENTS,
            "{hz:.3} Hz at {gain_db:.1} dBFS: {error:+.4} cents, clarity {:.4}", quiet.clarity
        );
        let drift = cents(quiet.frequency_hz, full.frequency_hz);
        assert!(
            drift.abs() <= 1e-4,
            "{hz:.3} Hz: {gain_db:.1} dBFS reads {drift:+.2e} cents from 0 dBFS"
        );
    }
}
