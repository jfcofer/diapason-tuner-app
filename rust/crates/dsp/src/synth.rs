//! Deterministic, seeded test signals: fixtures for the pitch detector's tests, and inputs for its
//! benches. Nothing else.
//!
//! **Never call these from the audio path**, the callback or anything it reaches. They do not
//! allocate, but they call `sin` and `exp` per sample and per partial, which costs far more than a
//! callback can afford. The module is public only so integration tests and benches can reach it.
//!
//! Synthetic fixtures are generated in code, never checked in (`docs/adr/0027`): the true frequency
//! is the generator's input, so a test can state it to any precision, and a failing case can be
//! regenerated from its parameters alone. Every generator is a pure function of its arguments, the
//! seed included, so two runs on one host produce bit-identical buffers. Across hosts they may
//! differ in the last bit, because `sin` and `exp` come from each platform's maths library; tests
//! that use them assert a tolerance, never exact samples.
//!
//! All phases are computed from the sample index (`θ = 2π·f·j/fs + φ`) rather than accumulated, so
//! there is no drift however long the buffer: the frequency of a fixture is exactly the one asked
//! for, to `f64` precision.

use std::f64::consts::{PI, TAU};

use crate::convert::{index_from_f64, index_to_f64, to_f32};

/// A sine, `amplitude · sin(2π·f·j/fs + phase)`, written over `out`.
///
/// Unlike [`crate::osc::SineOscillator`], which ramps its level in over
/// [`crate::osc::RAMP_SECONDS`] so that switching the test tone does not click, this starts at full
/// amplitude on the first sample: a ramp would be an amplitude modulation the detector should not
/// have to see in an accuracy fixture.
///
/// ```
/// let mut out = [0.0_f32; 4];
/// // A quarter of the sample rate: one cycle every four samples.
/// diapason_dsp::synth::sine(&mut out, 12_000.0, 48_000, 0.5, 0.0);
/// let expected = [0.0, 0.5, 0.0, -0.5];
/// assert!(out.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-7));
/// ```
pub fn sine(out: &mut [f32], frequency_hz: f64, sample_rate: u32, amplitude: f64, phase: f64) {
    let step = TAU * frequency_hz / f64::from(sample_rate.max(1));
    for (j, sample) in out.iter_mut().enumerate() {
        *sample = to_f32(amplitude * (step * index_to_f64(j) + phase).sin());
    }
}

/// How many harmonics of `frequency_hz` lie strictly below the Nyquist frequency:
/// `⌈(fs/2)/f⌉ − 1`, the largest `h` with `h·f < fs/2`.
///
/// Zero for a frequency that is zero, negative, infinite or NaN. Clamped to `fs/2` harmonics, the
/// count for 1 Hz, so that a tiny fundamental cannot ask [`sawtooth`] for an unbounded amount of
/// work; no fixture goes below 1 Hz.
///
/// ```
/// // 10 kHz at 48 kHz: 10 and 20 kHz are below 24 kHz, 30 kHz is not.
/// assert_eq!(diapason_dsp::synth::harmonics_below_nyquist(10_000.0, 48_000), 2);
/// assert_eq!(diapason_dsp::synth::harmonics_below_nyquist(12_000.0, 48_000), 1);
/// assert_eq!(diapason_dsp::synth::harmonics_below_nyquist(30_000.0, 48_000), 0);
/// ```
#[must_use]
pub fn harmonics_below_nyquist(frequency_hz: f64, sample_rate: u32) -> usize {
    if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
        return 0;
    }
    let nyquist = f64::from(sample_rate) / 2.0;
    let limit = usize::try_from(sample_rate / 2).unwrap_or(usize::MAX);
    index_from_f64((nyquist / frequency_hz).ceil() - 1.0).min(limit)
}

/// A band-limited sawtooth, built additively from every harmonic below Nyquist, written over `out`:
///
/// `x[j] = amplitude · (2/π) · Σₕ (−1)ʰ⁺¹ · sin(h·θⱼ) / h`, with `θⱼ = 2π·f·j/fs + phase`.
///
/// This is the Fourier series of a rising ramp from `−amplitude` to `+amplitude`, truncated before
/// Nyquist, so nothing aliases. The truncation overshoots by the Gibbs ≈ 9 % near each reset.
///
/// Per sample, `sin(h·θ)` for every `h` comes from the Chebyshev recurrence
/// `sin((h+1)θ) = 2·cos θ · sin(hθ) − sin((h−1)θ)`, seeded with one `sin` and one `cos`. Its rounding
/// error grows about linearly with `h`, so at the ≈ 800 harmonics of a 30 Hz tone at 48 kHz it stays
/// near 10⁻¹³, far below `f32` resolution.
///
/// ```
/// // At 10 kHz only the first two harmonics are below Nyquist.
/// let mut out = [0.0_f32; 8];
/// diapason_dsp::synth::sawtooth(&mut out, 10_000.0, 48_000, 1.0, 0.0);
/// let theta = std::f64::consts::TAU * 10_000.0 / 48_000.0;
/// let expected = 2.0 / std::f64::consts::PI * (theta.sin() - (2.0 * theta).sin() / 2.0);
/// assert!((f64::from(out[1]) - expected).abs() < 1e-6);
/// ```
pub fn sawtooth(out: &mut [f32], frequency_hz: f64, sample_rate: u32, amplitude: f64, phase: f64) {
    let harmonics = harmonics_below_nyquist(frequency_hz, sample_rate);
    let step = TAU * frequency_hz / f64::from(sample_rate.max(1));
    let scale = amplitude * 2.0 / PI;
    for (j, sample) in out.iter_mut().enumerate() {
        let theta = step * index_to_f64(j) + phase;
        let twice_cos = 2.0 * theta.cos();
        // sin(0·θ) and sin(1·θ); each pass moves the pair one harmonic up.
        let (mut previous, mut current) = (0.0, theta.sin());
        let mut sum = 0.0;
        let mut sign = 1.0;
        // `order` counts the harmonics in f64, exactly (they are far below 2⁵³).
        let mut order = 1.0;
        for _ in 0..harmonics {
            sum += sign * current / order;
            sign = -sign;
            order += 1.0;
            (previous, current) = (current, twice_cos * current - previous);
        }
        *sample = to_f32(scale * sum);
    }
}

/// The frequency of partial `n` (1-based) of a stiff string with fundamental parameter `f₀` and
/// inharmonicity coefficient `B`: `n · f₀ · √(1 + B·n²)` (Fletcher, *J. Acoust. Soc. Am.* 36, 1964).
///
/// Bending stiffness raises every partial above the harmonic series, the higher ones more. Steel
/// guitar strings have `B` around 10⁻⁵ to 10⁻⁴; wound bass strings reach 10⁻³. Note that even the
/// first partial sits at `f₀·√(1+B)`, not at `f₀`.
///
/// ```
/// use diapason_dsp::synth::partial_frequency;
/// assert_eq!(partial_frequency(110.0, 3, 0.0), 330.0);
/// assert!((partial_frequency(110.0, 10, 1e-4) - 1_100.0 * 1.01_f64.sqrt()).abs() < 1e-9);
/// ```
#[must_use]
pub fn partial_frequency(f0_hz: f64, n: usize, inharmonicity: f64) -> f64 {
    let n = index_to_f64(n);
    n * f0_hz * (1.0 + inharmonicity * n * n).sqrt()
}

/// Parameters of [`pluck`]'s stiff-string model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pluck {
    /// Inharmonicity coefficient `B` (see [`partial_frequency`]).
    pub inharmonicity: f64,
    /// Time constant, in seconds, of the first partial's exponential decay. Partial `n` decays `n`
    /// times faster: losses in a real string grow with frequency, which is why a pluck mellows.
    pub decay_seconds: f64,
    /// At most this many partials; any at or above Nyquist are left out regardless.
    pub max_partials: usize,
    /// Seeds the partials' starting phases.
    pub seed: u64,
}

impl Default for Pluck {
    /// A steel guitar string: `B = 10⁻⁴`, a 1 s fundamental decay, up to 40 partials.
    fn default() -> Self {
        Self {
            inharmonicity: 1e-4,
            decay_seconds: 1.0,
            max_partials: 40,
            seed: 1,
        }
    }
}

/// A plucked stiff string, written over `out`: partial `n` at [`partial_frequency`]`(f₀, n, B)`,
/// with amplitude `∝ 1/n`, decaying as `exp(−n·t / decay_seconds)`, from a seeded random phase.
///
/// The partial amplitudes are scaled so that their sum is `amplitude`, which bounds every sample by
/// it. This is a model of the *spectrum* a tuner sees after the attack (stretched partials, a
/// falling level, brightness that fades), not a physical simulation of the attack itself.
///
/// ```
/// use diapason_dsp::synth::{Pluck, pluck};
/// let mut a = vec![0.0_f32; 1_024];
/// let mut b = vec![0.0_f32; 1_024];
/// pluck(&mut a, 82.41, 48_000, 0.8, &Pluck::default());
/// pluck(&mut b, 82.41, 48_000, 0.8, &Pluck::default());
/// assert_eq!(a, b); // deterministic
/// assert!(a.iter().all(|s| s.abs() <= 0.8));
/// ```
pub fn pluck(out: &mut [f32], f0_hz: f64, sample_rate: u32, amplitude: f64, params: &Pluck) {
    let rate = f64::from(sample_rate.max(1));
    let nyquist = rate / 2.0;
    let partials = (1..=params.max_partials)
        .take_while(|&n| partial_frequency(f0_hz, n, params.inharmonicity) < nyquist)
        .count();
    let norm: f64 = (1..=partials).map(|n| 1.0 / index_to_f64(n)).sum();
    if partials == 0 {
        out.fill(0.0);
        return;
    }
    let decay_per_sample = 1.0 / (params.decay_seconds.max(f64::MIN_POSITIVE) * rate);

    // Sample by sample, so the partials are summed in f64 and narrowed once. Each sample replays
    // the seeded phase sequence from the start: partial n always takes the n-th draw.
    for (j, sample) in out.iter_mut().enumerate() {
        let t = index_to_f64(j);
        let mut phases = Noise::new(params.seed);
        let mut sum = 0.0;
        for n in 1..=partials {
            let order = index_to_f64(n);
            let step = TAU * partial_frequency(f0_hz, n, params.inharmonicity) / rate;
            let phase = PI * phases.next_f64();
            let level = amplitude / (order * norm) * (-order * decay_per_sample * t).exp();
            sum += level * (step * t + phase).sin();
        }
        *sample = to_f32(sum);
    }
}

/// Relative levels of partials 1, 2, 3 of a low string with a weak fundamental: the second
/// partial strongest, as a guitar's low E often is (`T-003b`'s reviewer's probe). A range that
/// cannot see the fundamental reads the second partial, at clarity 0.6: `(E_even − E_odd) / E`.
pub const WEAK_FUNDAMENTAL: [f64; 3] = [0.3, 1.0, 0.4];

/// Relative levels of partials 1–5 of a pinch harmonic: the fundamental weak, the second and third
/// partials strong (`T-003c`). Its NSDF at half the period and at a third are both 0.12, so only a
/// range that sees the fundamental reads it clearly, and as the fundamental.
pub const PINCH_HARMONIC: [f64; 5] = [0.2, 1.0, 0.9, 0.3, 0.1];

/// Relative levels of partials 1–4 of a five-string bass's low B (30.87 Hz): the fundamental
/// 16 dB under the second partial, the third and fourth strong (`T-003c`). A range that cannot see
/// 30.87 Hz reads the second partial at clarity 0.48, below what it may show.
pub const WEAK_LOW_B: [f64; 4] = [0.15, 1.0, 0.6, 0.3];

/// An additive tone, written over `out`: partial `n` (1-based) at `n · f`, with relative level
/// `levels[n − 1]` and phase `n · phase`, all scaled so that the levels sum to `amplitude`, which
/// bounds every sample by it. Harmonic, steady, with no partial at or above Nyquist (those are left
/// out). Summed in `f64` and narrowed once.
///
/// ```
/// use diapason_dsp::synth::{WEAK_FUNDAMENTAL, partials, sine};
/// let mut tone = [0.0_f32; 256];
/// partials(&mut tone, 110.0, 48_000, 0.5, 0.0, &[1.0]);
/// let mut reference = [0.0_f32; 256];
/// sine(&mut reference, 110.0, 48_000, 0.5, 0.0);
/// assert!(tone.iter().zip(&reference).all(|(a, b)| (a - b).abs() < 1e-7));
/// partials(&mut tone, 82.41, 48_000, 0.5, 0.3, &WEAK_FUNDAMENTAL);
/// assert!(tone.iter().all(|s| s.abs() <= 0.5));
/// ```
pub fn partials(
    out: &mut [f32],
    f0_hz: f64,
    sample_rate: u32,
    amplitude: f64,
    phase: f64,
    levels: &[f64],
) {
    let rate = f64::from(sample_rate.max(1));
    let total: f64 = levels.iter().map(|level| level.abs()).sum();
    let scale = if total > 0.0 { amplitude / total } else { 0.0 };
    let step = TAU * f0_hz / rate;
    for (j, sample) in out.iter_mut().enumerate() {
        let theta = step * index_to_f64(j);
        let mut sum = 0.0;
        let mut order = 1.0;
        for level in levels {
            if order * f0_hz < rate / 2.0 {
                sum += level * (order * (theta + phase)).sin();
            }
            order += 1.0;
        }
        *sample = to_f32(scale * sum);
    }
}

/// A seeded white-noise source: `SplitMix64` (Steele, Lea & Flood, OOPSLA 2014), mapped to a uniform
/// distribution on `[−1, 1)`.
///
/// Uniform rather than Gaussian because fixtures only need a flat spectrum and a stated power
/// (`1/3` per unit amplitude), and uniform samples are bounded, so they can never clip.
///
/// ```
/// use diapason_dsp::synth::Noise;
/// let (mut a, mut b) = (Noise::new(7), Noise::new(7));
/// assert_eq!(a.next_f64().to_bits(), b.next_f64().to_bits());
/// assert!((-1.0..1.0).contains(&a.next_f64()));
/// ```
#[derive(Debug, Clone)]
pub struct Noise {
    state: u64,
}

impl Noise {
    /// A source whose whole sequence is fixed by `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The next value, uniform on `[−1, 1)` with a resolution of 2⁻³¹.
    pub fn next_f64(&mut self) -> f64 {
        // The top 32 bits, exactly representable, scaled from [0, 2³²) to [−1, 1).
        let bits = u32::try_from(self.next_u64() >> 32).unwrap_or(0);
        f64::from(bits) / 2_147_483_648.0 - 1.0
    }

    /// Overwrite `out` with uniform noise of peak `amplitude` (RMS `amplitude/√3`).
    pub fn fill(&mut self, out: &mut [f32], amplitude: f64) {
        for sample in out {
            *sample = to_f32(amplitude * self.next_f64());
        }
    }
}

/// Mean power (mean square) of `samples`, in `f64`; zero for an empty slice.
///
/// ```
/// assert_eq!(diapason_dsp::synth::power(&[0.5, -0.5]), 0.25);
/// ```
#[must_use]
pub fn power(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    sum / index_to_f64(samples.len())
}

/// Add `noise` to `signal`, scaled so that the signal-to-noise ratio over their common length is
/// `snr_db`: `10·log₁₀(P_signal / (g²·P_noise)) = snr_db`. Returns the gain `g` applied to the noise,
/// zero (and `signal` untouched) if either has no power.
///
/// ```
/// use diapason_dsp::synth::{Noise, mix_at_snr, power, sine};
/// let mut signal = vec![0.0_f32; 4_800];
/// sine(&mut signal, 440.0, 48_000, 0.5, 0.0);
/// let mut noise = vec![0.0_f32; 4_800];
/// Noise::new(3).fill(&mut noise, 1.0);
/// let clean = power(&signal);
/// let gain = mix_at_snr(&mut signal, &noise, 20.0);
/// let snr = 10.0 * (clean / (gain * gain * power(&noise))).log10();
/// assert!((snr - 20.0).abs() < 1e-9);
/// ```
pub fn mix_at_snr(signal: &mut [f32], noise: &[f32], snr_db: f64) -> f64 {
    let len = signal.len().min(noise.len());
    let (signal, noise) = (
        signal.get_mut(..len).unwrap_or_default(),
        noise.get(..len).unwrap_or_default(),
    );
    let (p_signal, p_noise) = (power(signal), power(noise));
    if !(p_signal > 0.0 && p_noise > 0.0) {
        return 0.0;
    }
    let gain = (p_signal / (p_noise * 10_f64.powf(snr_db / 10.0))).sqrt();
    for (s, &n) in signal.iter_mut().zip(noise) {
        *s = to_f32(f64::from(*s) + gain * f64::from(n));
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_is_the_closed_form_from_the_first_sample() {
        let mut out = vec![0.0_f32; 2_000];
        sine(&mut out, 441.3, 44_100, 0.7, 1.0);
        for (j, &s) in out.iter().enumerate() {
            let t = index_to_f64(j) / 44_100.0;
            let expected = 0.7 * (TAU * 441.3 * t + 1.0).sin();
            assert!((f64::from(s) - expected).abs() < 1e-7, "sample {j}");
        }
        // No fade-in: full amplitude within the first period.
        assert!(out[..100].iter().any(|s| s.abs() > 0.69));
    }

    #[test]
    fn the_sawtooth_matches_its_truncated_fourier_series() {
        let (f, rate) = (1_234.5, 48_000);
        let harmonics = harmonics_below_nyquist(f, rate);
        assert_eq!(harmonics, 19); // 19 × 1234.5 = 23 455.5 < 24 000 < 20 × 1234.5
        let mut out = vec![0.0_f32; 500];
        sawtooth(&mut out, f, rate, 0.6, 0.3);
        for (j, &s) in out.iter().enumerate() {
            let theta = TAU * f * index_to_f64(j) / f64::from(rate) + 0.3;
            let expected: f64 = (1..=harmonics)
                .map(|h| {
                    let sign = if h % 2 == 1 { 1.0 } else { -1.0 };
                    let h = index_to_f64(h);
                    sign * (h * theta).sin() / h
                })
                .sum::<f64>()
                * 0.6
                * 2.0
                / PI;
            assert!((f64::from(s) - expected).abs() < 1e-6, "sample {j}");
        }
    }

    #[test]
    fn the_harmonic_count_is_the_closed_form_and_bounded() {
        for rate in [44_100_u32, 48_000] {
            let nyquist = f64::from(rate) / 2.0;
            for i in 0..2_000 {
                let f = 20.0 * 1.003_f64.powi(i);
                let brute = (1..=100_000)
                    .take_while(|&h| index_to_f64(h) * f < nyquist)
                    .count();
                assert_eq!(harmonics_below_nyquist(f, rate), brute, "{f} Hz at {rate}");
            }
        }
        for f in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(harmonics_below_nyquist(f, 48_000), 0, "{f}");
        }
        // A tiny fundamental is clamped to the count for 1 Hz instead of looping for ever.
        assert_eq!(harmonics_below_nyquist(1e-300, 48_000), 24_000);
        assert_eq!(harmonics_below_nyquist(f64::MIN_POSITIVE, 48_000), 24_000);
    }

    #[test]
    fn a_pluck_sums_its_partials_in_f64_and_rounds_once() {
        // 40 partials, each up to 1/norm in size. Summed in f32, each addition rounds, and the
        // error reaches several ulps; summed in f64 and narrowed once, it stays within half an ulp
        // of the f64 reference (6·10⁻⁸ for samples up to 1).
        let params = Pluck {
            inharmonicity: 1e-4,
            decay_seconds: 1.0,
            max_partials: 40,
            seed: 6,
        };
        let mut out = vec![0.0_f32; 4_800];
        pluck(&mut out, 110.0, 48_000, 1.0, &params);
        let norm: f64 = (1..=40).map(|n| 1.0 / index_to_f64(n)).sum();
        let mut reference = vec![0.0_f64; 4_800];
        let mut phases = Noise::new(6);
        for n in 1..=40 {
            let phase = PI * phases.next_f64();
            let f = partial_frequency(110.0, n, 1e-4);
            for (j, r) in reference.iter_mut().enumerate() {
                let t = index_to_f64(j) / 48_000.0;
                let order = index_to_f64(n);
                *r += (-order * t).exp() * (TAU * f * t + phase).sin() / (order * norm);
            }
        }
        let worst = out
            .iter()
            .zip(&reference)
            .map(|(&o, &r)| (f64::from(o) - r).abs())
            .fold(0.0, f64::max);
        assert!(worst <= 6.5e-8, "worst error {worst:e}");
    }

    #[test]
    fn a_low_sawtooth_is_a_ramp_away_from_its_reset() {
        // 30 Hz at 48 kHz: 799 harmonics, the most the fixtures use. Mid-period it is a straight
        // line from −A to +A over 1600 samples, to within the series' ripple.
        let mut out = vec![0.0_f32; 1_600];
        sawtooth(&mut out, 30.0, 48_000, 1.0, 0.0);
        // θ = 0 at j = 0 is the middle of the ramp: zero, rising 2/1600 per sample, with the reset
        // at θ = π (j = 800).
        for j in [200_usize, 400, 600, 1_000, 1_200] {
            let ideal = (index_to_f64(j) / 1_600.0) * 2.0;
            let ideal = if ideal > 1.0 { ideal - 2.0 } else { ideal };
            assert!((f64::from(out[j]) - ideal).abs() < 0.01, "sample {j}");
        }
    }

    #[test]
    fn one_partial_of_a_pluck_is_a_decaying_stretched_sine() {
        let params = Pluck {
            inharmonicity: 1e-3,
            decay_seconds: 0.1,
            max_partials: 1,
            seed: 9,
        };
        let mut out = vec![0.0_f32; 4_800];
        pluck(&mut out, 100.0, 48_000, 1.0, &params);
        let phase = PI * Noise::new(9).next_f64();
        let f1 = 100.0 * (1.0_f64 + 1e-3).sqrt();
        for (j, &s) in out.iter().enumerate() {
            let t = index_to_f64(j) / 48_000.0;
            let expected = (-t / 0.1).exp() * (TAU * f1 * t + phase).sin();
            assert!((f64::from(s) - expected).abs() < 1e-6, "sample {j}");
        }
    }

    #[test]
    fn a_pluck_decays_is_bounded_and_depends_on_its_seed() {
        let mut a = vec![0.0_f32; 48_000];
        pluck(&mut a, 110.0, 48_000, 0.9, &Pluck::default());
        assert!(a.iter().all(|s| s.abs() <= 0.9));
        // The fundamental alone loses e^(−2·0.83) ≈ 0.19 of its power between the two spans, and
        // every other partial loses more.
        assert!(power(&a[40_000..]) < power(&a[..8_000]) * 0.19);

        let mut b = vec![0.0_f32; 48_000];
        let reseeded = Pluck {
            seed: 2,
            ..Pluck::default()
        };
        pluck(&mut b, 110.0, 48_000, 0.9, &reseeded);
        assert_ne!(a, b);
    }

    #[test]
    fn partials_at_or_above_nyquist_are_left_out() {
        // 10 kHz: partial 2 is at 20 kHz·√(1+4B) < 24 kHz, partial 3 is above.
        let params = Pluck {
            inharmonicity: 1e-4,
            decay_seconds: 10.0,
            max_partials: 40,
            seed: 4,
        };
        let mut out = vec![0.0_f32; 64];
        pluck(&mut out, 10_000.0, 48_000, 1.0, &params);
        let mut phases = Noise::new(4);
        let (p1, p2) = (PI * phases.next_f64(), PI * phases.next_f64());
        let norm = 1.5;
        for (j, &s) in out.iter().enumerate() {
            let t = index_to_f64(j) / 48_000.0;
            let expected = (-t / 10.0).exp()
                * (TAU * partial_frequency(10_000.0, 1, 1e-4) * t + p1).sin()
                / norm
                + (-2.0 * t / 10.0).exp()
                    * (TAU * partial_frequency(10_000.0, 2, 1e-4) * t + p2).sin()
                    / (2.0 * norm);
            assert!((f64::from(s) - expected).abs() < 1e-6, "sample {j}");
        }
    }

    /// The autocorrelation of a steady harmonic tone at lag `τ`, normalised, is
    /// `Σ aₕ² cos(2πhτ/T) / Σ aₕ²`: what the fixtures' notes state about their NSDF.
    #[test]
    fn the_spectra_have_the_nsdf_their_notes_state() {
        // 100 Hz at 48 kHz: T = 480 samples, T/2 = 240, T/3 = 160, over 100 periods.
        let mut tone = vec![0.0_f32; 48_480];
        let at = |tone: &[f32], lag: usize| {
            let (head, tail) = (&tone[..48_000], &tone[lag..lag + 48_000]);
            let r: f64 = head
                .iter()
                .zip(tail)
                .map(|(&a, &b)| f64::from(a) * f64::from(b))
                .sum();
            r / head
                .iter()
                .map(|&a| f64::from(a) * f64::from(a))
                .sum::<f64>()
        };
        for (levels, half, third) in [
            (&WEAK_FUNDAMENTAL[..], 0.6, -0.31),
            (&PINCH_HARMONIC[..], 0.118, 0.123),
            (&WEAK_LOW_B[..], 0.480, -0.133),
        ] {
            partials(&mut tone, 100.0, 48_000, 0.5, 0.7, levels);
            assert!((at(&tone, 480) - 1.0).abs() < 1e-6, "{levels:?}");
            assert!(
                (at(&tone, 240) - half).abs() < 0.005,
                "{levels:?}: {}",
                at(&tone, 240)
            );
            assert!(
                (at(&tone, 160) - third).abs() < 0.005,
                "{levels:?}: {}",
                at(&tone, 160)
            );
        }
    }

    #[test]
    fn noise_is_seeded_uniform_and_flat() {
        let mut a = vec![0.0_f32; 100_000];
        Noise::new(42).fill(&mut a, 1.0);
        let mut b = vec![0.0_f32; 100_000];
        Noise::new(42).fill(&mut b, 1.0);
        assert_eq!(a, b);
        assert!(a.iter().all(|s| (-1.0..1.0).contains(s)));

        let mean: f64 = a.iter().map(|&s| f64::from(s)).sum::<f64>() / 100_000.0;
        // Uniform on [−1, 1): mean 0 and power 1/3, each with a standard error near 0.002 here.
        assert!(mean.abs() < 0.01, "mean {mean}");
        assert!((power(&a) - 1.0 / 3.0).abs() < 0.01, "power {}", power(&a));
        // White: the lag-one correlation is near zero.
        let lag1: f64 = a
            .windows(2)
            .map(|w| f64::from(w[0]) * f64::from(w[1]))
            .sum::<f64>()
            / 100_000.0;
        assert!((lag1 / power(&a)).abs() < 0.01, "lag-1 correlation {lag1}");

        Noise::new(43).fill(&mut b, 1.0);
        assert_ne!(a, b);
    }

    #[test]
    fn mixing_hits_the_requested_snr_and_ignores_silence() {
        let mut signal = vec![0.0_f32; 9_600];
        sawtooth(&mut signal, 220.0, 48_000, 0.3, 0.0);
        let clean = signal.clone();
        let mut noise = vec![0.0_f32; 9_600];
        Noise::new(5).fill(&mut noise, 1.0);
        for snr in [-10.0, 0.0, 20.0, 60.0] {
            let mut mixed = clean.clone();
            let gain = mix_at_snr(&mut mixed, &noise, snr);
            let measured = 10.0 * (power(&clean) / (gain * gain * power(&noise))).log10();
            assert!(
                (measured - snr).abs() < 1e-9,
                "asked {snr} dB, got {measured}"
            );
            assert_ne!(mixed, clean);
        }
        let silent = vec![0.0_f32; 9_600];
        assert_eq!(
            mix_at_snr(&mut signal, &silent, 0.0).to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(signal, clean);
    }
}
