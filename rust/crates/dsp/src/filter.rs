//! The tuner's linear filters (`docs/AUDIO_ENGINE.md` §2): a DC block, and the linear-phase FIR
//! low-pass that band-limits the full-rate path and anti-aliases the ×4 decimated one.
//!
//! Why the low-pass is there at all, and why it cuts so far below Nyquist: parabolic interpolation
//! of the NSDF peak is biased by harmonics near the *analysis* Nyquist (`T-003a`, Deviation 2: a
//! sawtooth's vertex lands up to 0.05 lag off at full band, 0.013 below `fs/4`, 0.0035 below
//! `fs/8`). An LTI filter keeps a periodic signal periodic with the same period, so removing
//! harmonics costs the detector nothing but that bias. Where each path cuts is decided in
//! [`crate::range`], from the probe recorded in `T-003b`.

use std::f64::consts::{PI, TAU};
use std::fmt;

use crate::convert::{index_from_f64, index_to_f64, to_f32};

/// Why a filter could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterError {
    /// The sample rate is zero, or not one the tuner's chain is designed for
    /// ([`crate::range::SUPPORTED_RATES`]).
    SampleRate(u32),
    /// The band edges are not `0 < passband < stopband < rate/2`.
    BandEdges {
        /// Passband edge, in hertz.
        passband_hz: f64,
        /// Stopband edge, in hertz.
        stopband_hz: f64,
    },
    /// The stopband attenuation is not in `21..=140` dB, the range Kaiser's formulas cover.
    Attenuation(f64),
    /// The design needs more than [`MAX_TAPS`] taps: the transition band is too narrow.
    TooManyTaps(usize),
    /// The decimation factor is zero.
    Factor(usize),
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SampleRate(rate) => write!(f, "unsupported sample rate {rate} Hz"),
            Self::BandEdges {
                passband_hz,
                stopband_hz,
            } => write!(
                f,
                "band edges {passband_hz} Hz and {stopband_hz} Hz are not 0 < pass < stop < rate/2"
            ),
            Self::Attenuation(db) => write!(f, "attenuation {db} dB is outside 21..=140"),
            Self::TooManyTaps(taps) => write!(f, "{taps} taps is more than {MAX_TAPS}"),
            Self::Factor(factor) => write!(f, "decimation factor {factor} is not at least 1"),
        }
    }
}

impl std::error::Error for FilterError {}

/// A one-pole, one-zero DC blocker: `y[n] = x[n] − x[n−1] + R·y[n−1]`.
///
/// The zero at `z = 1` removes DC exactly; the pole at `z = R`, with `R = exp(−2π·f_c/fs)` and
/// `f_c` = [`DcBlock::CUTOFF_HZ`], sets where the response turns over, so handling rumble goes
/// and low B (30.87 Hz) keeps most of its level. The magnitude response is
///
/// ```text
///   |H(e^{jω})| = |1 − e^{−jω}| / |1 − R·e^{−jω}| = 2·|sin(ω/2)| / √(1 − 2R·cos ω + R²)
/// ```
///
/// and a step decays as `Rⁿ`, so by −90 dB after `⌈ln(10^{−90/20}) / ln R⌉` samples: 3958 at
/// 48 kHz, 82.5 ms, ten time constants of `1/(2π·20 Hz)` ([`DcBlock::settling_samples`]).
///
/// State is `f64`: at 48 kHz `1 − R = 2.6·10⁻³`, and in `f32` the recursion would round that
/// small difference coarsely enough to move the corner. An output state below
/// [`DcBlock::FLUSH`] is set to exactly zero, so silence after a signal never decays into
/// subnormals, which cost tens of cycles per operation on some cores. A non-finite output (a NaN
/// or an infinity in the input) resets both states, so it costs one sample, not the stream.
/// Allocation-free and panic-free.
///
/// ```
/// use diapason_dsp::filter::DcBlock;
///
/// let mut dc = DcBlock::new(48_000)?;
/// let mut block = [1.0_f32; 8_000]; // a DC step
/// dc.process(&mut block);
/// assert!(block[7_999].abs() < 1e-4); // gone after a sixth of a second
/// # Ok::<(), diapason_dsp::filter::FilterError>(())
/// ```
#[derive(Debug, Clone)]
pub struct DcBlock {
    sample_rate: u32,
    coefficient: f64,
    previous_input: f64,
    previous_output: f64,
}

impl DcBlock {
    /// The corner frequency, in hertz (`docs/AUDIO_ENGINE.md` §2).
    pub const CUTOFF_HZ: f64 = 20.0;

    /// Output states smaller than this in magnitude are flushed to zero. It is far below anything
    /// audible (−600 dBFS) and far above `f64`'s subnormals (below 2.2·10⁻³⁰⁸); it is also above
    /// `f32`'s (below 1.2·10⁻³⁸), so the narrowed output is never subnormal either.
    pub const FLUSH: f64 = 1e-30;

    /// A DC block for `sample_rate`.
    ///
    /// # Errors
    ///
    /// [`FilterError::SampleRate`] if the rate is not above `2·CUTOFF_HZ`.
    pub fn new(sample_rate: u32) -> Result<Self, FilterError> {
        if f64::from(sample_rate) <= 2.0 * Self::CUTOFF_HZ {
            return Err(FilterError::SampleRate(sample_rate));
        }
        Ok(Self {
            sample_rate,
            coefficient: (-TAU * Self::CUTOFF_HZ / f64::from(sample_rate)).exp(),
            previous_input: 0.0,
            previous_output: 0.0,
        })
    }

    /// The pole, `R`.
    #[must_use]
    pub fn coefficient(&self) -> f64 {
        self.coefficient
    }

    /// `|H|` at `frequency_hz`, from the closed form above, at the rate the block was built for.
    #[must_use]
    pub fn magnitude(&self, frequency_hz: f64) -> f64 {
        let omega = TAU * frequency_hz / f64::from(self.sample_rate);
        let r = self.coefficient;
        2.0 * (omega / 2.0).sin().abs() / (1.0 - 2.0 * r * omega.cos() + r * r).sqrt()
    }

    /// Samples for a step to decay by `attenuation_db`: the smallest `n` with `Rⁿ ≤ 10^{−dB/20}`.
    #[must_use]
    pub fn settling_samples(&self, attenuation_db: f64) -> usize {
        let target = -attenuation_db / 20.0 * std::f64::consts::LN_10;
        index_from_f64((target / self.coefficient.ln()).ceil())
    }

    /// Filter one sample.
    #[inline]
    pub fn process_sample(&mut self, sample: f32) -> f32 {
        let input = f64::from(sample);
        let mut output = input - self.previous_input + self.coefficient * self.previous_output;
        if !output.is_finite() {
            // A NaN or an infinity would otherwise stay in the recursion for ever: drop it, and
            // the state with it, and carry on from silence.
            self.previous_input = 0.0;
            self.previous_output = 0.0;
            return 0.0;
        }
        if output.abs() < Self::FLUSH {
            output = 0.0;
        }
        self.previous_input = input;
        self.previous_output = output;
        to_f32(output)
    }

    /// Filter `block` in place.
    pub fn process(&mut self, block: &mut [f32]) {
        for sample in block {
            *sample = self.process_sample(*sample);
        }
    }

    /// The recursion's output state, `y[n−1]`. Exactly zero after enough silence.
    #[must_use]
    pub fn state(&self) -> f64 {
        self.previous_output
    }
}

/// The modified Bessel function of the first kind, order zero, from its power series
///
/// ```text
///   I₀(x) = Σ_{k≥0} ((x/2)ᵏ / k!)²
/// ```
///
/// Every term is positive, so there is no cancellation; summation stops once a term falls below
/// the sum's last bit (about 40 terms at `x = 20`, more than any Kaiser `β` here needs). Abramowitz
/// & Stegun 9.6.12. Local because `dsp` takes no dependency for one function (`tools/check-deps.sh`).
///
/// ```
/// use diapason_dsp::filter::bessel_i0;
/// assert_eq!(bessel_i0(0.0), 1.0);
/// assert!((bessel_i0(1.0) - 1.266_065_877_752_008_4).abs() < 1e-15);
/// ```
#[must_use]
pub fn bessel_i0(x: f64) -> f64 {
    let half = x / 2.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..=500_u32 {
        let ratio = half / f64::from(k);
        term *= ratio * ratio;
        sum += term;
        if term <= sum * f64::EPSILON {
            break;
        }
    }
    sum
}

/// The most taps [`kaiser_low_pass`] will design. The tuner's filters need a few hundred.
pub const MAX_TAPS: usize = 4_095;

/// A low-pass design: pass below `passband_hz`, attenuate by `attenuation_db` above
/// `stopband_hz`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LowPassSpec {
    /// Sample rate of the filter's input, in hertz.
    pub sample_rate: u32,
    /// Passband edge, in hertz.
    pub passband_hz: f64,
    /// Stopband edge, in hertz.
    pub stopband_hz: f64,
    /// Stopband attenuation, and passband ripple `±10^{−A/20}`, in dB.
    pub attenuation_db: f64,
}

/// The taps of a linear-phase low-pass FIR: a Kaiser-windowed ideal low-pass (Kaiser, "Nonrecursive
/// digital filter design using the I₀-sinh window function", *Proc. IEEE ISCAS* 1974; Oppenheim &
/// Schafer, *Discrete-Time Signal Processing*, 3rd ed., §7.6).
///
/// With `A` the attenuation and `Δω = 2π·(f_stop − f_pass)/fs` the transition width:
///
/// ```text
///   β = 0.1102·(A − 8.7)                               A > 50
///     = 0.5842·(A − 21)^0.4 + 0.07886·(A − 21)         21 ≤ A ≤ 50
///   M = ⌈(A − 8) / (2.285·Δω)⌉, rounded up to even      taps N = M + 1, odd
///   h[n] = 2f_c/fs · sinc(2f_c/fs · (n − M/2)) · I₀(β√(1 − (2n/M − 1)²)) / I₀(β)
/// ```
///
/// with `f_c` midway between the edges. `M` even makes the group delay `M/2` a whole number of
/// samples, which keeps the latency ledger an integer. The taps are symmetric, `h[n] = h[M − n]`,
/// which is what makes the phase linear, and are scaled to sum to 1 (unity gain at DC). The
/// formulas are approximate, so the tests measure the result rather than trust them.
///
/// # Errors
///
/// [`FilterError`] for a zero rate, band edges out of order or beyond Nyquist, an attenuation
/// outside `21..=140` dB, or a design longer than [`MAX_TAPS`].
pub fn kaiser_low_pass(spec: &LowPassSpec) -> Result<Vec<f64>, FilterError> {
    let rate = f64::from(spec.sample_rate);
    if spec.sample_rate == 0 {
        return Err(FilterError::SampleRate(spec.sample_rate));
    }
    let (pass, stop) = (spec.passband_hz, spec.stopband_hz);
    if !(pass > 0.0 && pass < stop && stop < rate / 2.0) {
        return Err(FilterError::BandEdges {
            passband_hz: pass,
            stopband_hz: stop,
        });
    }
    let attenuation = spec.attenuation_db;
    if !(21.0..=140.0).contains(&attenuation) {
        return Err(FilterError::Attenuation(attenuation));
    }
    let beta = if attenuation > 50.0 {
        0.1102 * (attenuation - 8.7)
    } else {
        0.5842 * (attenuation - 21.0).powf(0.4) + 0.07886 * (attenuation - 21.0)
    };
    let transition = TAU * (stop - pass) / rate;
    let order = index_from_f64(((attenuation - 8.0) / (2.285 * transition)).ceil());
    let order = order + order % 2;
    if order + 1 > MAX_TAPS {
        return Err(FilterError::TooManyTaps(order + 1));
    }
    let cutoff = (pass + stop) / rate; // 2·f_c/fs
    let middle = index_to_f64(order) / 2.0;
    let norm = bessel_i0(beta);
    let mut taps: Vec<f64> = (0..=order)
        .map(|n| {
            let t = index_to_f64(n) - middle;
            let x = t / middle;
            let window = bessel_i0(beta * (1.0 - x * x).max(0.0).sqrt()) / norm;
            cutoff * sinc(cutoff * t) * window
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    for tap in &mut taps {
        *tap /= sum;
    }
    Ok(taps)
}

/// `sin(πx)/(πx)`, 1 at 0.
fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// A linear-phase FIR low-pass that keeps one output in every `factor` inputs.
///
/// Only the kept outputs are computed: output `k` is the filter's response at input `k·M + M − 1`
/// (`M` the factor), the dot product of the taps with the `N` most recent inputs. That is exactly
/// the work of a polyphase decimator (the `M` sub-filters `h[p + iM]` each see every `M`-th input,
/// and their sum is the same dot product), written in direct form because the history is
/// contiguous. The symmetric taps are folded, `h[k]·(x[n−k] + x[n−N+1+k])`, which halves the
/// multiplications.
///
/// With a factor of 1 it is the full-rate path's band-limit: one output per input, same filter
/// arithmetic.
///
/// The history is a ring written twice, at `i` and `i + N`, so the `N` newest samples are always
/// one contiguous slice: no wrap-around test in the inner loop. Every output depends only on the
/// input sequence, so it is the same for any split into blocks. Taps, history and sums are `f64`.
/// Allocation-free and panic-free after [`Decimator::new`].
///
/// ```
/// use diapason_dsp::filter::{Decimator, LowPassSpec};
///
/// let spec = LowPassSpec {
///     sample_rate: 48_000,
///     passband_hz: 1_000.0,
///     stopband_hz: 3_000.0,
///     attenuation_db: 80.0,
/// };
/// let mut decimator = Decimator::new(&spec, 4)?;
/// let mut outputs = 0;
/// decimator.process(&[0.25; 400], |_| outputs += 1);
/// assert_eq!(outputs, 100);
/// assert_eq!(decimator.group_delay(), (decimator.taps().len() - 1) / 2);
/// # Ok::<(), diapason_dsp::filter::FilterError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Decimator {
    taps: Vec<f64>,
    factor: usize,
    /// `2N` samples; the newest `N` are `history[position..position + N]`, oldest first.
    history: Vec<f64>,
    /// Where the next sample goes, in `0..N`.
    position: usize,
    /// Inputs since the last output, in `0..factor`.
    phase: usize,
}

impl Decimator {
    /// A decimator by `factor` through the low-pass `spec` designs ([`kaiser_low_pass`]).
    ///
    /// # Errors
    ///
    /// [`FilterError::Factor`] for a zero factor, or whatever the design rejects.
    pub fn new(spec: &LowPassSpec, factor: usize) -> Result<Self, FilterError> {
        if factor == 0 {
            return Err(FilterError::Factor(factor));
        }
        let taps = kaiser_low_pass(spec)?;
        let history = vec![0.0; 2 * taps.len()];
        Ok(Self {
            taps,
            factor,
            history,
            position: 0,
            phase: 0,
        })
    }

    /// The taps, `h[0..N)`.
    #[must_use]
    pub fn taps(&self) -> &[f64] {
        &self.taps
    }

    /// Inputs per output.
    #[must_use]
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The group delay, `(N − 1)/2`, in *input* samples: a whole number, because `N` is odd.
    #[must_use]
    pub fn group_delay(&self) -> usize {
        (self.taps.len() - 1) / 2
    }

    /// Forget every input: the state of a new decimator.
    pub fn reset(&mut self) {
        self.history.fill(0.0);
        self.position = 0;
        self.phase = 0;
    }

    /// Take one input; return an output on every `factor`-th.
    #[inline]
    pub fn process_sample(&mut self, sample: f32) -> Option<f32> {
        let len = self.taps.len();
        let value = f64::from(sample);
        if let Some(slot) = self.history.get_mut(self.position) {
            *slot = value;
        }
        if let Some(slot) = self.history.get_mut(self.position + len) {
            *slot = value;
        }
        self.position += 1;
        if self.position == len {
            self.position = 0;
        }
        self.phase += 1;
        if self.phase < self.factor {
            return None;
        }
        self.phase = 0;
        Some(self.convolve())
    }

    /// Take every sample of `input`, in order, and hand each output to `emit`.
    pub fn process(&mut self, input: &[f32], mut emit: impl FnMut(f32)) {
        for &sample in input {
            if let Some(output) = self.process_sample(sample) {
                emit(output);
            }
        }
    }

    /// `Σ h[k]·x[n−k]` over the `N` newest samples, folded on the taps' symmetry.
    fn convolve(&self) -> f32 {
        let len = self.taps.len();
        let window = self
            .history
            .get(self.position..self.position + len)
            .unwrap_or_default();
        let half = len / 2;
        let (oldest, rest) = window.split_at(half.min(window.len()));
        let (centre, newest) = rest.split_first().unwrap_or((&0.0, &[]));
        // `window` is oldest first, so the newest sample meets h[0] and the oldest h[N−1] = h[0].
        let mut sum = self.taps.get(half).copied().unwrap_or(0.0) * centre;
        for ((tap, old), new) in self.taps.iter().zip(oldest).zip(newest.iter().rev()) {
            sum += tap * (old + new);
        }
        to_f32(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;
    use proptest::prelude::*;

    #[test]
    fn bessel_i0_matches_reference_values() {
        // Abramowitz & Stegun, Table 9.8, and mpmath to 17 digits.
        for (x, expected) in [
            (0.5, 1.063_483_370_741_323_6),
            (1.0, 1.266_065_877_752_008_4),
            (5.0, 27.239_871_823_604_442),
            (10.0, 2_815.716_628_466_254),
        ] {
            let got = bessel_i0(x);
            assert!(
                ((got - expected) / expected).abs() < 1e-14,
                "I0({x}) = {got}, expected {expected}"
            );
        }
        assert_eq!(bessel_i0(-2.0).to_bits(), bessel_i0(2.0).to_bits());
    }

    #[test]
    fn a_dc_step_decays_below_minus_90_db_within_the_documented_time() {
        let mut dc = DcBlock::new(48_000).unwrap();
        let samples = dc.settling_samples(90.0);
        // ln(10^{−4.5}) / ln R with R = exp(−2π·20/48000): 10.3616 / 2.61799·10⁻³ = 3957.9.
        assert_eq!(samples, 3_958);
        let mut step = vec![1.0_f32; samples + 1];
        dc.process(&mut step);
        let threshold = 10_f32.powf(-90.0 / 20.0);
        assert!(step[samples].abs() <= threshold, "{}", step[samples]);
        assert!(step[samples - 40].abs() > threshold); // and not much sooner
        // 44.1 kHz: the same 82.5 ms.
        assert_eq!(DcBlock::new(44_100).unwrap().settling_samples(90.0), 3_637);
    }

    #[test]
    fn the_response_at_low_b_matches_the_closed_form() {
        for rate in [44_100, 48_000] {
            let mut dc = DcBlock::new(rate).unwrap();
            let hz = 30.87;
            // Two seconds to settle, then the RMS over 30 whole periods (rounded to a sample, a
            // relative error near 10⁻⁵).
            let settle = 2 * usize::try_from(rate).unwrap();
            let periods = index_from_f64((30.0 * f64::from(rate) / hz).round());
            let mut tone = vec![0.0_f32; settle + periods];
            sine(&mut tone, hz, rate, 0.5, 0.0);
            dc.process(&mut tone);
            let tail = &tone[settle..];
            let rms = (crate::synth::power(tail) * 2.0).sqrt() / 0.5;
            let expected = dc.magnitude(hz);
            assert!(
                (rms - expected).abs() < 2e-5,
                "{rate} Hz: measured {rms}, closed form {expected}"
            );
            // About −1.5 dB: 1/√(1 + (20/30.87)²) for the analogue prototype.
            assert!((expected - 0.839).abs() < 2e-3, "{expected}");
        }
        assert_eq!(DcBlock::new(0).unwrap_err(), FilterError::SampleRate(0));
    }

    #[test]
    fn a_nan_or_an_infinity_does_not_poison_later_output() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut dc = DcBlock::new(48_000).unwrap();
            let mut block = vec![0.0_f32; 960];
            sine(&mut block, 440.0, 48_000, 0.5, 0.0);
            block[100] = bad;
            dc.process(&mut block);
            assert_eq!(block[100].to_bits(), 0.0_f32.to_bits());
            assert!(block.iter().all(|s| s.is_finite()), "{bad}");
            assert!(dc.state().is_finite());
            // And the tone is back at its level once the restart has settled.
            assert!(block[900..].iter().any(|s| s.abs() > 0.4));
        }
    }

    #[test]
    fn silence_after_a_signal_returns_the_state_to_exactly_zero() {
        let mut dc = DcBlock::new(48_000).unwrap();
        let mut tone = vec![0.0_f32; 4_800];
        sine(&mut tone, 440.0, 48_000, 0.9, 0.3);
        dc.process(&mut tone);
        assert!(dc.state() != 0.0);
        // R^n reaches 1e-30 after ln(1e-30)/ln(R) ≈ 26 400 samples; one second is plenty.
        let mut silence = vec![0.0_f32; 48_000];
        dc.process(&mut silence);
        assert_eq!(dc.state().to_bits(), 0.0_f64.to_bits());
        assert!(silence.iter().all(|s| !s.is_subnormal()));
        assert_eq!(silence[47_999].to_bits(), 0.0_f32.to_bits());
    }

    /// Every output of `decimator` for `input`, in a buffer sized up front (the crate's RT lint
    /// list denies `Vec::push`, in tests too).
    fn outputs(decimator: &mut Decimator, input: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0_f32; input.len()];
        let mut len = 0;
        decimator.process(input, |y| {
            out[len] = y;
            len += 1;
        });
        out.truncate(len);
        out
    }

    fn test_spec() -> LowPassSpec {
        LowPassSpec {
            sample_rate: 48_000,
            passband_hz: 1_000.0,
            stopband_hz: 3_000.0,
            attenuation_db: 80.0,
        }
    }

    #[test]
    fn taps_are_symmetric_odd_in_number_and_sum_to_one() {
        let taps = kaiser_low_pass(&test_spec()).unwrap();
        assert_eq!(taps.len() % 2, 1);
        for (a, b) in taps.iter().zip(taps.iter().rev()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        assert!((taps.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_designs_are_rejected() {
        let ok = test_spec();
        let edges = |passband_hz, stopband_hz| {
            kaiser_low_pass(&LowPassSpec {
                passband_hz,
                stopband_hz,
                ..ok
            })
        };
        assert!(matches!(
            edges(0.0, 1_000.0),
            Err(FilterError::BandEdges { .. })
        ));
        assert!(matches!(
            edges(2_000.0, 1_000.0),
            Err(FilterError::BandEdges { .. })
        ));
        assert!(matches!(
            edges(1_000.0, 24_000.0),
            Err(FilterError::BandEdges { .. })
        ));
        assert!(matches!(
            edges(1_000.0, 1_001.0),
            Err(FilterError::TooManyTaps(_))
        ));
        assert_eq!(
            kaiser_low_pass(&LowPassSpec {
                attenuation_db: 10.0,
                ..ok
            }),
            Err(FilterError::Attenuation(10.0))
        );
        assert_eq!(
            kaiser_low_pass(&LowPassSpec {
                sample_rate: 0,
                ..ok
            }),
            Err(FilterError::SampleRate(0))
        );
        assert_eq!(Decimator::new(&ok, 0).unwrap_err(), FilterError::Factor(0));
    }

    #[test]
    fn the_folded_convolution_equals_the_direct_one() {
        let mut decimator = Decimator::new(&test_spec(), 1).unwrap();
        let mut input = vec![0.0_f32; 1_000];
        crate::synth::Noise::new(3).fill(&mut input, 0.5);
        let taps = decimator.taps().to_vec();
        for (n, &got) in outputs(&mut decimator, &input).iter().enumerate() {
            let direct: f64 = taps
                .iter()
                .enumerate()
                .filter_map(|(k, h)| n.checked_sub(k).map(|i| h * f64::from(input[i])))
                .sum();
            assert!((f64::from(got) - direct).abs() < 1e-6, "n = {n}");
        }
    }

    proptest! {
        /// Outputs depend on the input only, never on how it was cut into blocks.
        #[test]
        fn output_is_identical_for_any_block_split(
            blocks in prop::collection::vec(1_usize..300, 1..30),
            factor in 1_usize..=4,
        ) {
            let total: usize = blocks.iter().sum();
            let mut input = vec![0.0_f32; total];
            crate::synth::Noise::new(9).fill(&mut input, 0.7);
            let mut whole = Decimator::new(&test_spec(), factor).unwrap();
            let expected = outputs(&mut whole, &input);

            let mut split = Decimator::new(&test_spec(), factor).unwrap();
            let mut got = Vec::new();
            let mut start = 0;
            for size in blocks {
                got.extend(outputs(&mut split, &input[start..start + size]));
                start += size;
            }
            let bits = |v: &[f32]| v.iter().map(|y| y.to_bits()).collect::<Vec<_>>();
            assert_eq!(bits(&got), bits(&expected));
            assert_eq!(expected.len(), total / factor);
        }
    }
}
