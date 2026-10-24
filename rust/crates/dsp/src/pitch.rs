//! Pitch detection by the `McLeod` Pitch Method (MPM) (`docs/AUDIO_ENGINE.md` §3, `docs/adr/0005`).
//!
//! P. `McLeod` and G. Wyvill, "A smarter way to find pitch", *Proc. ICMC* 2005. For a frame
//! `x[0..N)`, and a lag `τ`:
//!
//! ```text
//!   r(τ) = Σ_{j=0}^{N−1−τ} x[j]·x[j+τ]                    autocorrelation
//!   m(τ) = Σ_{j=0}^{N−1−τ} (x[j]² + x[j+τ]²)              energy of the two overlapping parts
//!   n(τ) = 2·r(τ) / m(τ)                                   the NSDF
//! ```
//!
//! `m(τ) − 2r(τ) = Σ (x[j] − x[j+τ])²` is the square difference function, which is never negative,
//! and `m(τ) + 2r(τ) = Σ (x[j] + x[j+τ])²` likewise, so `n(τ) ∈ [−1, 1]`; `n(τ) = 1` exactly when the
//! frame repeats with period `τ`. Unlike plain autocorrelation, `n` does not lean towards small lags
//! as the overlap shrinks, because `m` shrinks with it.

use std::fmt;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::convert::{index_from_f64, index_to_f64, to_f32};

/// The shortest window [`Mpm`] accepts. Below this there are too few lags to find a key maximum and
/// its two neighbours.
pub const MIN_WINDOW: usize = 16;

/// The longest window [`Mpm`] accepts: 2²⁰ samples, about 22 s at 48 kHz, far beyond any analysis
/// window (the longest in `docs/AUDIO_ENGINE.md` §2 is 1200 samples). The bound keeps the FFT length and every
/// lag exactly representable in `f64` (see [`crate::convert::index_to_f64`]).
pub const MAX_WINDOW: usize = 1 << 20;

/// Configuration of an [`Mpm`] detector. Fixed for its lifetime, because the window decides the size
/// of every buffer and FFT plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MpmConfig {
    /// Samples per analysed frame, `N`. Lags up to `N/2` are searched, so the longest period found
    /// is `N/2` samples; in practice a window should hold three periods of the lowest note
    /// (`docs/AUDIO_ENGINE.md` §2).
    pub window: usize,
    /// `k` in step 4: the first key maximum reaching `k · max(n)` is the period. Lower values are
    /// more sensitive and more prone to octave errors. In `(0, 1]`.
    pub threshold: f64,
    /// Estimates whose clarity is below this are discarded (step 6). In `[0, 1]`.
    pub clarity_floor: f32,
}

impl MpmConfig {
    /// The default thresholds (`k = 0.9`, clarity floor 0.6) with the given window.
    #[must_use]
    pub fn with_window(window: usize) -> Self {
        Self {
            window,
            ..Self::default()
        }
    }
}

impl Default for MpmConfig {
    /// A 4096-sample window with `k = 0.9` and a clarity floor of 0.6 (`docs/AUDIO_ENGINE.md` §3).
    fn default() -> Self {
        Self {
            window: 4_096,
            threshold: 0.9,
            clarity_floor: 0.6,
        }
    }
}

/// Why an [`MpmConfig`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MpmError {
    /// The window is outside [`MIN_WINDOW`]`..=`[`MAX_WINDOW`].
    Window(usize),
    /// The threshold `k` is not in `(0, 1]`.
    Threshold(f64),
    /// The clarity floor is not in `[0, 1]`.
    ClarityFloor(f32),
}

impl fmt::Display for MpmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(window) => write!(
                f,
                "window of {window} samples is outside {MIN_WINDOW}..={MAX_WINDOW}"
            ),
            Self::Threshold(k) => write!(f, "threshold {k} is outside (0, 1]"),
            Self::ClarityFloor(floor) => write!(f, "clarity floor {floor} is outside [0, 1]"),
        }
    }
}

impl std::error::Error for MpmError {}

/// One frame's pitch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchEstimate {
    /// The fundamental frequency, in hertz: the sample rate over [`Self::period_samples`].
    pub frequency_hz: f64,
    /// The period in samples, with its sub-sample part from parabolic interpolation (step 5).
    pub period_samples: f64,
    /// The NSDF's interpolated value at the period (step 6): 1 for a perfectly periodic frame,
    /// falling with noise, inharmonicity and change within the frame. At least the configured
    /// clarity floor, and at most 1.
    pub clarity: f32,
}

/// A `McLeod` Pitch Method detector for one window length.
///
/// Every buffer and both FFT plans are allocated by [`Mpm::new`]; [`Mpm::analyze`] neither allocates
/// nor panics, so it may run on any thread, though by design it runs on the analysis thread, not in
/// the audio callback (`docs/AUDIO_ENGINE.md` §2).
///
/// The FFTs and the NSDF run in `f64`. Measured in `T-003a` on the host bench: `f32` FFTs are 25–29 %
/// faster (2048–8192 windows), but where the NSDF peak is flattest, at low notes, their rounding
/// grows the error twentyfold (0.0005 to 0.011 cents at 30–60 Hz) and makes the estimate depend on
/// the input gain by up to 0.003 cents. The frame cost is a small fraction of its budget either way.
///
/// ```
/// use diapason_dsp::pitch::{Mpm, MpmConfig};
/// use diapason_dsp::synth::sine;
///
/// let mut mpm = Mpm::new(MpmConfig::with_window(2_048))?;
/// let mut frame = vec![0.0_f32; 2_048];
/// sine(&mut frame, 440.0, 48_000, 0.5, 0.0);
/// let estimate = mpm.analyze(&frame, 48_000).expect("a clean sine is clear");
/// assert!((estimate.frequency_hz - 440.0).abs() < 0.01);
/// assert!(estimate.clarity > 0.99);
/// # Ok::<(), diapason_dsp::pitch::MpmError>(())
/// ```
pub struct Mpm {
    config: MpmConfig,
    /// The largest lag searched, `N/2`.
    max_lag: usize,
    forward: Arc<dyn RealToComplex<f64>>,
    inverse: Arc<dyn ComplexToReal<f64>>,
    /// The frame, zero-padded to the FFT length. The forward FFT uses it as scratch.
    padded: Vec<f64>,
    /// `X[k]`, then `|X[k]|²`; the inverse FFT uses it as scratch.
    spectrum: Vec<Complex<f64>>,
    /// `L · r(τ)` for every lag of the FFT length `L` (the inverse is unnormalised).
    autocorrelation: Vec<f64>,
    /// `n(τ)` for `τ ∈ [0, max_lag]`.
    nsdf: Vec<f64>,
    /// `m(τ)` for `τ ∈ [0, max_lag]`, kept for step 5's common overlap.
    overlap_energy: Vec<f64>,
    forward_scratch: Vec<Complex<f64>>,
    inverse_scratch: Vec<Complex<f64>>,
}

impl fmt::Debug for Mpm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Mpm")
            .field("config", &self.config)
            .field("fft_len", &self.padded.len())
            .field("max_lag", &self.max_lag)
            .finish_non_exhaustive()
    }
}

impl Mpm {
    /// A detector for `config`, with every buffer and FFT plan it will need.
    ///
    /// The FFT length `L` is the smallest power of two `≥ 2N`. Zero-padding to at least `2N − 1`
    /// is what makes the FFT's circular correlation equal the *linear* `r(τ)` above: the circular
    /// result at lag `τ` is `r(τ) + r(L − τ)`, and `r` vanishes beyond `N − 1`.
    ///
    /// # Errors
    ///
    /// [`MpmError`] if the window, threshold or clarity floor is out of range.
    pub fn new(config: MpmConfig) -> Result<Self, MpmError> {
        if !(MIN_WINDOW..=MAX_WINDOW).contains(&config.window) {
            return Err(MpmError::Window(config.window));
        }
        if !(config.threshold > 0.0 && config.threshold <= 1.0) {
            return Err(MpmError::Threshold(config.threshold));
        }
        if !(0.0..=1.0).contains(&config.clarity_floor) {
            return Err(MpmError::ClarityFloor(config.clarity_floor));
        }
        let fft_len = (2 * config.window).next_power_of_two();
        let mut planner = RealFftPlanner::<f64>::new();
        let forward = planner.plan_fft_forward(fft_len);
        let inverse = planner.plan_fft_inverse(fft_len);
        let max_lag = config.window / 2;
        Ok(Self {
            config,
            max_lag,
            padded: vec![0.0; fft_len],
            spectrum: vec![Complex::new(0.0, 0.0); forward.complex_len()],
            autocorrelation: vec![0.0; fft_len],
            nsdf: vec![0.0; max_lag + 1],
            overlap_energy: vec![0.0; max_lag + 1],
            forward_scratch: vec![Complex::new(0.0, 0.0); forward.get_scratch_len()],
            inverse_scratch: vec![Complex::new(0.0, 0.0); inverse.get_scratch_len()],
            forward,
            inverse,
        })
    }

    /// The configuration this detector was built for.
    #[must_use]
    pub fn config(&self) -> &MpmConfig {
        &self.config
    }

    /// The pitch of `frame`, sampled at `sample_rate`, or `None` if it has none clear enough.
    ///
    /// `None` also covers every input this detector cannot judge: a frame whose length is not the
    /// configured window, a zero sample rate, silence, and non-finite samples. Allocation-free and
    /// panic-free.
    pub fn analyze(&mut self, frame: &[f32], sample_rate: u32) -> Option<PitchEstimate> {
        if frame.len() != self.config.window || sample_rate == 0 {
            return None;
        }
        let energy = self.autocorrelate(frame)?;
        self.normalise(frame, energy);
        let lag = self.pick()?;
        let [a, b, c] = self.common_overlap(frame, lag)?;
        let (offset, peak) = vertex(a, b, c)?;
        let clarity = to_f32(peak.min(1.0));
        if clarity < self.config.clarity_floor {
            return None;
        }
        let period_samples = index_to_f64(lag) + offset;
        Some(PitchEstimate {
            frequency_hz: f64::from(sample_rate) / period_samples,
            period_samples,
            clarity,
        })
    }

    /// The largest lag searched, `N/2`: a period longer than `max_lag() − 1` samples has no peak
    /// with a neighbour on each side, so this detector cannot see it.
    #[must_use]
    pub fn max_lag(&self) -> usize {
        self.max_lag
    }

    /// The NSDF's peak nearest `lag`, as an estimate, from the frame [`Self::analyze`] last judged:
    /// the octave guard's test of `τ/2` and `2τ` (`docs/AUDIO_ENGINE.md` §3, `crate::octave`).
    ///
    /// The peak is the highest local maximum of `n` at `round(lag)` or one lag either side (the
    /// sub-sample period doubled or halved lands within one lag of its sample). It is then refined
    /// exactly as the chosen period is, steps 5 and 6, over its common overlap, so its clarity
    /// compares like for like with [`PitchEstimate::clarity`]. O(1): it reuses the NSDF, `r` and
    /// `m` that `analyze` kept, and no second FFT runs. The clarity floor is *not* applied.
    ///
    /// `frame` must be the frame `analyze` last returned `Some` for; with any other the answer is
    /// meaningless (but still allocation-free and panic-free). `None` where there is no local
    /// maximum near `lag`, or `lag` has no neighbour on each side within the searched lags.
    #[must_use]
    pub fn peak_near(&self, frame: &[f32], sample_rate: u32, lag: f64) -> Option<PitchEstimate> {
        if frame.len() != self.config.window || sample_rate == 0 || lag.is_nan() || lag < 1.0 {
            return None;
        }
        let centre = index_from_f64(lag.round());
        // Only the local-maximum test is needed: no scan for the first zero crossing, so O(1).
        let maxima = KeyMaxima {
            nsdf: &self.nsdf,
            lag: 0,
        };
        let peak = [centre.saturating_sub(1), centre, centre.saturating_add(1)]
            .into_iter()
            .filter(|&tau| tau < self.max_lag && maxima.is_local_maximum(tau))
            .max_by(|&a, &b| maxima.value(a).total_cmp(&maxima.value(b)))?;
        let [a, b, c] = self.common_overlap(frame, peak)?;
        let (offset, value) = vertex(a, b, c)?;
        let period_samples = index_to_f64(peak) + offset;
        Some(PitchEstimate {
            frequency_hz: f64::from(sample_rate) / period_samples,
            period_samples,
            clarity: to_f32(value.min(1.0)),
        })
    }

    /// The NSDF `n` (step 3) at `round(lag)`, from the frame [`Self::analyze`] last judged: a level
    /// where there may be no peak, for the octave guard's view of the octave above (`crate::octave`).
    /// O(1). `None` outside the searched lags or for a lag that is not a number.
    #[must_use]
    pub fn value_at(&self, lag: f64) -> Option<f64> {
        if lag.is_nan() || lag < 0.0 {
            return None;
        }
        self.nsdf.get(index_from_f64(lag.round())).copied()
    }

    /// Step 1: `L · r(τ)` into `self.autocorrelation`, through the Wiener–Khinchin theorem: the
    /// autocorrelation is the inverse transform of the power spectrum `|X[k]|²`. Returns the frame's
    /// energy `Σ x²`, or `None` for silence, non-finite input or an FFT error (which the buffer
    /// sizes rule out; it is still not a reason to panic).
    fn autocorrelate(&mut self, frame: &[f32]) -> Option<f64> {
        let (head, tail) = self.padded.split_at_mut_checked(frame.len())?;
        let mut energy = 0.0;
        for (padded, &sample) in head.iter_mut().zip(frame) {
            let sample = f64::from(sample);
            *padded = sample;
            energy += sample * sample;
        }
        tail.fill(0.0);
        if !(energy.is_finite() && energy > 0.0) {
            return None;
        }

        self.forward
            .process_with_scratch(
                &mut self.padded,
                &mut self.spectrum,
                &mut self.forward_scratch,
            )
            .ok()?;
        for bin in &mut self.spectrum {
            *bin = Complex::new(bin.norm_sqr(), 0.0);
        }
        self.inverse
            .process_with_scratch(
                &mut self.spectrum,
                &mut self.autocorrelation,
                &mut self.inverse_scratch,
            )
            .ok()?;
        Some(energy)
    }

    /// Steps 2 and 3: `n(τ)` for every searched lag.
    ///
    /// `m` is updated in O(1) per lag: going from `τ` to `τ + 1` drops the first sample of the
    /// leading part and the last sample of the trailing part, so `m(0) = 2·Σx²` and
    /// `m(τ+1) = m(τ) − x[τ]² − x[N−1−τ]²`. The inverse FFT is unnormalised, hence `r = acf / L`.
    ///
    /// The division is always safe. `m(τ) = Σ_{j ≤ N−1−τ} x[j]² + Σ_{j ≥ τ} x[j]²`, and for the
    /// searched lags, `τ ≤ N/2`, the two ranges together cover every sample (`N − 1 − τ ≥ τ − 1`).
    /// So `m(τ) ≥ Σx² > 0`, which [`Self::autocorrelate`] checked. The recurrence's rounding is at
    /// most about `N·ε·2Σx²`, some 10⁻¹² of that bound for the longest window.
    fn normalise(&mut self, frame: &[f32], energy: f64) {
        let scale = 1.0 / index_to_f64(self.padded.len());
        let mut m = 2.0 * energy;
        let lags = self.nsdf.iter_mut().zip(&mut self.overlap_energy);
        let lags = lags.zip(&self.autocorrelation);
        for (((n, kept), &acf), (&head, &tail)) in lags.zip(frame.iter().zip(frame.iter().rev())) {
            *n = 2.0 * acf * scale / m;
            *kept = m;
            m -= f64::from(head) * f64::from(head) + f64::from(tail) * f64::from(tail);
        }
    }

    /// Step 4: the lag of the first key maximum that reaches `k` times the highest one.
    ///
    /// Picking the first rather than the highest is what avoids octave-down errors: the NSDF at
    /// 2T, 3T, … is nearly as high as at T for a steady tone. `k` below 1 tolerates a period peak a
    /// little lower than a later one; too low, and a strong harmonic's peak at T/2 wins instead.
    fn pick(&self) -> Option<usize> {
        let highest = KeyMaxima::new(&self.nsdf)
            .map(|(_, value)| value)
            .fold(f64::NEG_INFINITY, f64::max);
        let bar = self.config.threshold * highest;
        KeyMaxima::new(&self.nsdf)
            .find(|&(_, value)| value >= bar)
            .map(|(lag, _)| lag)
    }

    /// Step 5's three values, `n̄(τ)` at `τ = lag − 1, lag, lag + 1`, recomputed over **one common
    /// overlap** of `M = N − lag − 1` pairs, counted from both ends of the frame (`T-003b`):
    ///
    /// ```text
    ///   n̄(τ) = 2·(Σ_{j<M} x[j]·x[j+τ] + Σ_{i≥N−M} x[i−τ]·x[i])
    ///          / (Σ_{j<M} (x[j]² + x[j+τ]²) + Σ_{i≥N−M} (x[i−τ]² + x[i]²))
    /// ```
    ///
    /// Why. The FFT NSDF sums `N − τ` pairs at lag `τ`, so its three samples around the peak come from
    /// three *different* sums: each lag drops a pair at the window's edge. When that pair sits on a
    /// steep part of the waveform (a sawtooth's reset), the peak turns lopsided and the parabola's
    /// vertex moves. It is worst when the window holds a whole number of periods, because then the
    /// pairs dropped at both edges sit at the same point of the cycle: the sawtooth missed by 0.5 ¢
    /// at exactly 75, 100 and 150 Hz in the Mid range.
    ///
    /// With one set of pairs for all three lags, the three values are samples of a single smooth
    /// function of `τ` whose maximum is exactly the period: there `x[j+T] = x[j]`, so it equals 1, and
    /// Cauchy–Schwarz bounds it by 1 everywhere. Counting from the start only, that function is still
    /// lopsided, because its later samples slide while its earlier ones stay put; counting from the
    /// end, the opposite holds. Summing both cancels the lopsidedness to first order. On the chain's
    /// headline grid (`tests/chain.rs`: 24 points per octave plus every whole-period frequency
    /// ±0.3 Hz, 8 phases) the sawtooth's worst is −0.507 ¢ without it (75 Hz, 44.1 kHz, Mid) and
    /// 0.087 ¢ with it (`T-003b`, `docs/adr/0028`).
    ///
    /// It costs O(1), not O(N): each common-overlap sum is the full `r(τ)` or `m(τ)` of steps 1–2
    /// minus the pairs beyond the overlap, and at `τ = lag − 1, lag, lag + 1` there are `2, 1, 0` of
    /// those at each end (`j ∈ [M, N − 1 − τ]` from the start, `j ∈ [0, lag − τ]` from the end).
    /// A test checks it against the definition above.
    ///
    /// `None` if a lag has no neighbour on either side, or the overlap has no energy.
    fn common_overlap(&self, frame: &[f32], lag: usize) -> Option<[f64; 3]> {
        let span = frame.len().checked_sub(lag.checked_add(1)?)?;
        if lag == 0 || span == 0 {
            return None;
        }
        let scale = 1.0 / index_to_f64(self.padded.len());
        let mut values = [0.0; 3];
        for (value, tau) in values.iter_mut().zip([lag - 1, lag, lag + 1]) {
            let full_product = *self.autocorrelation.get(tau)? * scale;
            let full_energy = *self.overlap_energy.get(tau)?;
            let (mut product, mut energy) = (2.0 * full_product, 2.0 * full_energy);
            for extra in 0..lag + 1 - tau {
                for j in [span + extra, extra] {
                    let a = f64::from(*frame.get(j)?);
                    let b = f64::from(*frame.get(j + tau)?);
                    product -= a * b;
                    energy -= a * a + b * b;
                }
            }
            if energy.is_nan() || energy <= 0.0 {
                return None;
            }
            *value = 2.0 * product / energy;
        }
        Some(values)
    }
}

/// Steps 5 and 6: the vertex of the parabola through `(−1, a)`, `(0, b)`, `(1, c)`, as its offset
/// from the middle point and the value there.
///
/// The vertex is at `δ = (a − c) / (2(a − 2b + c))`, where the parabola takes the value
/// `b − (a − c)·δ / 4`. At a key maximum `|δ| ≤ ½`; values from [`Mpm::common_overlap`] can lean a
/// little further. A vertex outside the bracket, `|δ| > 1`, is no peak at all (the middle value is
/// not the highest), so it gives `None` and the frame is discarded rather than clamped to an edge.
///
/// The vertex is exact for a parabola, and within 7.1·10⁻⁴ lag (0.041 cents) of a sine's
/// `cos(ωτ)` peak at any lag of 30 or more. It is biased on a sharper peak: a sawtooth's NSDF
/// peak is `Σ wₕ cos(hω(τ − T))` with `wₕ ∝ 1/h²`, so every harmonic up to Nyquist adds equally
/// to its curvature, and the vertex lands up to 0.053 lag from `T` whatever the window. The bias
/// depends on how close the signal's content reaches Nyquist (0.013 lag below `fs/4`, 0.0035
/// below `fs/8`), which is why the tuner's chain band-limits (`crate::range`).
fn vertex(a: f64, b: f64, c: f64) -> Option<(f64, f64)> {
    let curvature = a - 2.0 * b + c;
    if curvature.is_nan() || curvature >= 0.0 {
        // Flat: the vertex is the sample itself.
        return Some((0.0, b));
    }
    let offset = 0.5 * (a - c) / curvature;
    if offset.abs() > 1.0 {
        return None;
    }
    Some((offset, b - 0.25 * (a - c) * offset))
}

/// The key maxima of an NSDF (step 4): for each region where `n > 0`, after the one around lag 0,
/// the lag and value of its highest local maximum.
///
/// The region around lag 0 is skipped because `n(0) = 1` trivially; it ends where `n` first drops
/// to zero or below. A region that runs into the last lag yields its highest *interior* local
/// maximum, if it has one: a value still rising at the boundary is not a peak, and could not be
/// interpolated. A NaN counts as not positive, so it ends a region and every scan moves past it:
/// the iterator always terminates.
struct KeyMaxima<'a> {
    nsdf: &'a [f64],
    lag: usize,
}

impl<'a> KeyMaxima<'a> {
    fn new(nsdf: &'a [f64]) -> Self {
        let lag = nsdf
            .iter()
            .position(|&n| !positive(n))
            .unwrap_or(nsdf.len());
        Self { nsdf, lag }
    }

    fn value(&self, lag: usize) -> f64 {
        self.nsdf.get(lag).copied().unwrap_or(f64::NEG_INFINITY)
    }

    /// Strictly above its left neighbour and at least its right one, so a plateau counts once.
    fn is_local_maximum(&self, lag: usize) -> bool {
        let Some(left) = lag.checked_sub(1) else {
            return false;
        };
        let here = self.value(lag);
        lag + 1 < self.nsdf.len() && here > self.value(left) && here >= self.value(lag + 1)
    }
}

/// Strictly above zero; false for NaN, which `<= 0.0` would also call false and so never skip.
fn positive(value: f64) -> bool {
    value > 0.0
}

impl Iterator for KeyMaxima<'_> {
    type Item = (usize, f64);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while self.lag < self.nsdf.len() && !positive(self.value(self.lag)) {
                self.lag += 1;
            }
            if self.lag >= self.nsdf.len() {
                return None;
            }
            let mut best: Option<Self::Item> = None;
            while self.lag < self.nsdf.len() && positive(self.value(self.lag)) {
                let value = self.value(self.lag);
                if self.is_local_maximum(self.lag) && best.is_none_or(|(_, top)| value > top) {
                    best = Some((self.lag, value));
                }
                self.lag += 1;
            }
            if best.is_some() {
                return best;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{Noise, sine};

    fn nsdf_of(frame: &[f32]) -> Vec<f64> {
        let mut mpm = Mpm::new(MpmConfig::with_window(frame.len())).unwrap();
        let energy = mpm.autocorrelate(frame).unwrap();
        mpm.normalise(frame, energy);
        mpm.nsdf.clone()
    }

    /// `n(τ)` straight from its definition, in O(N²).
    fn nsdf_direct(frame: &[f32], max_lag: usize) -> Vec<f64> {
        let x: Vec<f64> = frame.iter().map(|&s| f64::from(s)).collect();
        (0..=max_lag)
            .map(|tau| {
                let pairs = x.iter().zip(&x[tau..]);
                let r: f64 = pairs.clone().map(|(a, b)| a * b).sum();
                let m: f64 = pairs.map(|(a, b)| a * a + b * b).sum();
                2.0 * r / m
            })
            .collect()
    }

    #[test]
    fn the_fft_nsdf_equals_the_definition() {
        for (window, seed) in [(16, 1), (100, 2), (512, 3)] {
            let mut frame = vec![0.0_f32; window];
            Noise::new(seed).fill(&mut frame, 0.5);
            let mut tone = vec![0.0_f32; window];
            sine(&mut tone, 2_000.0, 48_000, 0.5, 0.2);
            for (f, t) in frame.iter_mut().zip(&tone) {
                *f += t;
            }
            let fast = nsdf_of(&frame);
            let slow = nsdf_direct(&frame, window / 2);
            for (tau, (a, b)) in fast.iter().zip(&slow).enumerate() {
                assert!(
                    (a - b).abs() < 1e-12,
                    "window {window}, lag {tau}: {a} vs {b}"
                );
                assert!((-1.0..=1.0 + 1e-12).contains(a));
            }
        }
    }

    /// Step 5's common overlap straight from its definition, in O(N).
    fn common_overlap_direct(frame: &[f32], lag: usize) -> [f64; 3] {
        let (len, span) = (frame.len(), frame.len() - lag - 1);
        let x: Vec<f64> = frame.iter().map(|&s| f64::from(s)).collect();
        [lag - 1, lag, lag + 1].map(|tau| {
            let pairs = (0..span).map(|j| (x[j], x[j + tau]));
            let pairs = pairs.chain((len - span..len).map(|i| (x[i - tau], x[i])));
            let (r, m) = pairs.fold((0.0, 0.0), |(r, m), (a, b)| (r + a * b, m + a * a + b * b));
            2.0 * r / m
        })
    }

    #[test]
    fn the_o1_common_overlap_equals_its_definition() {
        for (window, seed) in [(16, 1), (100, 2), (513, 3), (1_200, 4)] {
            let mut frame = vec![0.0_f32; window];
            Noise::new(seed).fill(&mut frame, 0.3);
            let mut tone = vec![0.0_f32; window];
            sine(&mut tone, 700.0, 12_000, 0.5, 0.4);
            for (f, t) in frame.iter_mut().zip(&tone) {
                *f += t;
            }
            let mut mpm = Mpm::new(MpmConfig::with_window(window)).unwrap();
            let energy = mpm.autocorrelate(&frame).unwrap();
            mpm.normalise(&frame, energy);
            for lag in [1, 2, window / 4, window / 2 - 1] {
                let fast = mpm.common_overlap(&frame, lag).unwrap();
                let slow = common_overlap_direct(&frame, lag);
                for (a, b) in fast.iter().zip(slow) {
                    assert!(
                        (a - b).abs() < 1e-12,
                        "window {window}, lag {lag}: {a} vs {b}"
                    );
                }
            }
        }
    }

    #[test]
    fn key_maxima_skip_the_zero_lobe_and_take_one_peak_per_positive_region() {
        let nsdf = [
            1.0, 0.5, -0.1, 0.3, 0.6, 0.4, 0.7, 0.2, -0.2, 0.1, 0.9, 0.8, -0.5, 0.2, 0.4,
        ];
        let maxima: Vec<_> = KeyMaxima::new(&nsdf).collect();
        // The last region is still rising at the end: not a peak.
        assert_eq!(maxima, [(6, 0.7), (10, 0.9)]);
        assert_eq!(KeyMaxima::new(&[1.0, 0.9, 0.8]).count(), 0);
    }

    #[test]
    fn a_plateau_is_one_key_maximum_at_its_first_lag() {
        let nsdf = [1.0, -0.1, 0.5, 0.8, 0.8, 0.3, -0.2];
        assert_eq!(KeyMaxima::new(&nsdf).collect::<Vec<_>>(), [(3, 0.8)]);
    }

    #[test]
    fn a_nan_ends_a_region_and_never_stalls_the_scan() {
        let nan = f64::NAN;
        let nsdf = [1.0, 0.4, nan, 0.3, 0.7, 0.2, nan, nan, 0.1, 0.5, 0.2, -0.1];
        assert_eq!(
            KeyMaxima::new(&nsdf).collect::<Vec<_>>(),
            [(4, 0.7), (9, 0.5)]
        );
        assert_eq!(KeyMaxima::new(&[nan; 8]).count(), 0);
        assert_eq!(KeyMaxima::new(&[1.0, nan, nan, 0.5, nan]).count(), 0);
    }

    /// Key maxima at lags 4 (0.7), 8 (0.95) and 12 (0.9).
    const THREE_PEAKS: [f64; 17] = [
        1.0, 0.5, -0.1, 0.3, 0.7, 0.2, -0.2, 0.4, 0.95, 0.3, -0.3, 0.1, 0.9, 0.2, -0.1, -0.2, -0.3,
    ];

    #[test]
    fn the_configured_threshold_decides_which_key_maximum_is_the_period() {
        for (threshold, lag) in [(0.9, 8), (1.0, 8), (0.7, 4), (0.5, 4)] {
            let mut mpm = Mpm::new(MpmConfig {
                threshold,
                ..MpmConfig::with_window(32)
            })
            .unwrap();
            mpm.nsdf.copy_from_slice(&THREE_PEAKS);
            assert_eq!(mpm.pick(), Some(lag), "k = {threshold}");
        }
    }

    #[test]
    fn a_low_threshold_takes_a_strong_second_harmonic_for_the_period() {
        // x = sin θ + 3·sin 2θ. Its NSDF at T/2 is about (−1·1 + 9·1)/(1 + 9) = 0.8 of the peak
        // at T: below k = 0.9 · 1, above k = 0.75 · 1.
        let mut frame = vec![0.0_f32; 1_024];
        let mut octave = vec![0.0_f32; 1_024];
        sine(&mut frame, 200.0, 48_000, 0.2, 0.0);
        sine(&mut octave, 400.0, 48_000, 0.6, 0.0);
        for (f, o) in frame.iter_mut().zip(&octave) {
            *f += o;
        }
        let read = |threshold| {
            let config = MpmConfig {
                threshold,
                ..MpmConfig::with_window(1_024)
            };
            Mpm::new(config).unwrap().analyze(&frame, 48_000).unwrap()
        };
        assert!((read(0.9).frequency_hz - 200.0).abs() < 0.01);
        // T/2 is not a period of the frame, so its peak is lower and a little off centre (400.64
        // Hz here); what matters is the octave.
        let low = read(0.75);
        assert!((low.frequency_hz / 400.0 - 1.0).abs() < 0.01, "{low:?}");
    }

    #[test]
    fn the_parabola_through_a_parabola_is_exact() {
        let mut mpm = Mpm::new(MpmConfig::with_window(16)).unwrap();
        let top = 4.3;
        for (tau, n) in mpm.nsdf.iter_mut().enumerate() {
            let d = index_to_f64(tau) - top;
            *n = 0.9 - 0.05 * d * d;
        }
        let n = |lag: usize| mpm.nsdf[lag];
        let (offset, peak) = vertex(n(3), n(4), n(5)).unwrap();
        assert!((offset - 0.3).abs() < 1e-12 && (peak - 0.9).abs() < 1e-12);
        // The middle value well below one neighbour: the vertex is 4.5 lags out, no peak at all.
        assert_eq!(vertex(0.9, 0.5, 0.0), None);
        assert!(vertex(0.6, 0.62, 0.0).is_some());
        // A lag with no neighbour below, or no pairs left above, has no common overlap.
        let mut frame = [0.0_f32; 16];
        crate::synth::Noise::new(2).fill(&mut frame, 0.5);
        let energy = mpm.autocorrelate(&frame).unwrap();
        mpm.normalise(&frame, energy);
        assert_eq!(mpm.common_overlap(&frame, 0), None);
        assert_eq!(mpm.common_overlap(&frame, 15), None);
        assert!(mpm.common_overlap(&frame, 7).is_some());
    }

    /// `peak_near`, the octave guard's τ/2 and 2τ test, on real frames; the ratios are those
    /// `crate::octave::COMPARABLE` is set between.
    #[test]
    fn peak_near_reads_the_octaves_of_the_frame_just_analysed() {
        let rate = 12_000;
        // Low's shape, 1200 samples at 12 kHz, so 2T of 110 Hz (218 lags) is searched.
        let mut mpm = Mpm::new(MpmConfig::with_window(1_200)).unwrap();
        let mut frame = vec![0.0_f32; 1_200];
        crate::synth::partials(&mut frame, 110.0, rate, 0.5, 0.3, &[1.0, 0.5, 0.33, 0.25]);
        let pick = mpm.analyze(&frame, rate).unwrap();
        assert!((pick.frequency_hz - 110.0).abs() < 0.01);
        // The octave below a steady note is a period too: as clear as the note.
        let below = mpm
            .peak_near(&frame, rate, 2.0 * pick.period_samples)
            .unwrap();
        assert!((below.frequency_hz - 55.0).abs() < 0.01, "{below:?}");
        assert!(below.clarity >= 0.999 * pick.clarity, "{below:?} {pick:?}");
        // A weak fundamental's second partial: about 0.6 of the note, not comparable.
        crate::synth::partials(
            &mut frame,
            82.41,
            rate,
            0.5,
            0.3,
            &crate::synth::WEAK_FUNDAMENTAL,
        );
        let pick = mpm.analyze(&frame, rate).unwrap();
        let up = mpm
            .peak_near(&frame, rate, pick.period_samples / 2.0)
            .unwrap();
        assert!((up.frequency_hz / 164.82 - 1.0).abs() < 0.003, "{up:?}");
        let ratio = up.clarity / pick.clarity;
        assert!((0.55..0.65).contains(&ratio), "{ratio}");
        // A sawtooth's NSDF is at a minimum at T/2: no peak there.
        crate::synth::sawtooth(&mut frame, 110.0, rate, 0.5, 0.0);
        let pick = mpm.analyze(&frame, rate).unwrap();
        assert_eq!(mpm.peak_near(&frame, rate, pick.period_samples / 2.0), None);
        // Beyond the searched lags, at no lag, or for a frame it cannot judge: none.
        assert_eq!(mpm.peak_near(&frame, rate, 6.0 * pick.period_samples), None);
        for lag in [f64::NAN, 0.0, -5.0, f64::INFINITY] {
            assert_eq!(mpm.peak_near(&frame, rate, lag), None, "{lag}");
        }
        assert_eq!(
            mpm.peak_near(&frame[..1_199], rate, pick.period_samples),
            None
        );
        assert_eq!(mpm.peak_near(&frame, 0, pick.period_samples), None);
        assert_eq!(mpm.max_lag(), 600);
    }

    #[test]
    fn invalid_configurations_are_rejected() {
        let ok = MpmConfig::default();
        for (config, error) in [
            (MpmConfig { window: 15, ..ok }, MpmError::Window(15)),
            (
                MpmConfig {
                    window: MAX_WINDOW + 1,
                    ..ok
                },
                MpmError::Window(MAX_WINDOW + 1),
            ),
            (
                MpmConfig {
                    threshold: 0.0,
                    ..ok
                },
                MpmError::Threshold(0.0),
            ),
            (
                MpmConfig {
                    threshold: 1.1,
                    ..ok
                },
                MpmError::Threshold(1.1),
            ),
            (
                MpmConfig {
                    clarity_floor: -0.1,
                    ..ok
                },
                MpmError::ClarityFloor(-0.1),
            ),
        ] {
            assert_eq!(Mpm::new(config).unwrap_err(), error);
        }
        assert!(
            Mpm::new(MpmConfig {
                threshold: f64::NAN,
                ..ok
            })
            .is_err()
        );
        assert!(Mpm::new(MpmConfig::with_window(MIN_WINDOW)).is_ok());
    }

    #[test]
    fn frames_it_cannot_judge_give_none() {
        let mut mpm = Mpm::new(MpmConfig::with_window(256)).unwrap();
        let mut frame = vec![0.0_f32; 256];
        assert_eq!(mpm.analyze(&frame, 48_000), None); // silence
        sine(&mut frame, 1_000.0, 48_000, 0.5, 0.0);
        assert!(mpm.analyze(&frame, 48_000).is_some());
        assert_eq!(mpm.analyze(&frame, 0), None);
        assert_eq!(mpm.analyze(&frame[..255], 48_000), None);
        frame[17] = f32::NAN;
        assert_eq!(mpm.analyze(&frame, 48_000), None);
        frame[17] = f32::INFINITY;
        assert_eq!(mpm.analyze(&frame, 48_000), None);
    }

    #[test]
    fn a_constant_frame_has_no_pitch() {
        // n(τ) = 1 everywhere: no zero crossing, so no key maximum.
        let mut mpm = Mpm::new(MpmConfig::with_window(256)).unwrap();
        assert_eq!(mpm.analyze(&[0.25; 256], 48_000), None);
    }
}
