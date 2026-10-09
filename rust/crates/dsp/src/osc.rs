//! A sine test tone that stays phase-continuous across blocks of any size.

use std::f64::consts::TAU;

use crate::convert::to_f32;

/// How long a full-scale level change (silence to amplitude 1) takes; smaller changes are
/// proportionally quicker. Long enough that switching the tone on or off does not click,
/// short enough to feel instant.
pub const RAMP_SECONDS: f64 = 0.005;

/// A sine oscillator with a click-free level ramp.
///
/// Phase is an `f64` count of cycles in `[0, 1)`, advanced once per sample, so the output depends
/// only on how many samples have been rendered and never on how they were split into blocks: two
/// renders of the same length are bit-identical whatever their block sizes. Changing the frequency
/// keeps the phase, so the waveform never jumps. Allocation-free and panic-free on every path.
///
/// ```
/// use diapason_dsp::osc::SineOscillator;
///
/// let mut tone = SineOscillator::new(48_000);
/// tone.set_frequency(440.0);
/// tone.set_amplitude(0.5);
/// let mut block = [0.0_f32; 480];
/// tone.fill(&mut block);
/// let peak = |samples: &[f32]| samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
/// // The first quarter period (~27 samples) rises under the ramp, far below the level reached
/// // later: the tone fades in rather than clicking on.
/// assert!(peak(&block[..28]) < peak(&block[370..]) / 2.0);
/// assert!(block.iter().all(|s| s.abs() <= 0.5));
/// ```
#[derive(Debug, Clone)]
pub struct SineOscillator {
    sample_rate: f64,
    phase: f64,
    increment: f64,
    level: f32,
    target: f32,
    ramp_step: f32,
}

impl SineOscillator {
    /// A silent oscillator at 0 Hz for a stream running at `sample_rate`.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = f64::from(sample_rate.max(1));
        Self {
            sample_rate,
            phase: 0.0,
            increment: 0.0,
            level: 0.0,
            target: 0.0,
            ramp_step: to_f32(1.0 / (RAMP_SECONDS * sample_rate).max(1.0)),
        }
    }

    /// Set the frequency in hertz, keeping the current phase. Clamped to `[0, Nyquist]`; a
    /// non-finite value silences the pitch rather than corrupting the phase.
    pub fn set_frequency(&mut self, hz: f32) {
        let hz = f64::from(hz);
        let hz = if hz.is_finite() {
            hz.clamp(0.0, self.sample_rate / 2.0)
        } else {
            0.0
        };
        self.increment = hz / self.sample_rate;
    }

    /// The frequency in hertz, after clamping.
    #[must_use]
    pub fn frequency(&self) -> f32 {
        to_f32(self.increment * self.sample_rate)
    }

    /// Set the peak amplitude to ramp towards, clamped to `[0, 1]`. Zero fades the tone out.
    pub fn set_amplitude(&mut self, amplitude: f32) {
        self.target = if amplitude.is_finite() {
            amplitude.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// The amplitude being ramped towards.
    #[must_use]
    pub fn amplitude(&self) -> f32 {
        self.target
    }

    /// Overwrite `out` with the next `out.len()` samples.
    pub fn fill(&mut self, out: &mut [f32]) {
        for sample in out {
            self.level = approach(self.level, self.target, self.ramp_step);
            *sample = self.level * to_f32((TAU * self.phase).sin());
            self.phase += self.increment;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

/// Move `current` one `step` towards `target`, without overshooting it.
fn approach(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Frequency from upward zero crossings, each located by linear interpolation. Independent of
    /// the oscillator: it only looks at the samples.
    fn measured_hz(samples: &[f32], sample_rate: u32) -> f64 {
        let mut first = None;
        let mut last = 0.0;
        let mut cycles = 0_u32;
        for (index, pair) in (0_u32..).zip(samples.windows(2)) {
            let (a, b) = (f64::from(pair[0]), f64::from(pair[1]));
            if a < 0.0 && b >= 0.0 {
                let crossing = f64::from(index) + a / (a - b);
                if first.is_none() {
                    first = Some(crossing);
                } else {
                    cycles += 1;
                }
                last = crossing;
            }
        }
        let span = last - first.unwrap_or(last);
        f64::from(cycles) * f64::from(sample_rate) / span
    }

    fn render(sample_rate: u32, hz: f32, amplitude: f32, seconds: u32) -> Vec<f32> {
        let mut tone = SineOscillator::new(sample_rate);
        tone.set_frequency(hz);
        tone.set_amplitude(amplitude);
        let mut out = vec![0.0; (sample_rate * seconds) as usize];
        tone.fill(&mut out);
        out
    }

    #[test]
    fn frequency_is_accurate_to_a_thousandth_of_a_hertz() {
        for sample_rate in [44_100, 48_000] {
            for hz in [82.41_f32, 440.0, 1_318.51] {
                let measured = measured_hz(&render(sample_rate, hz, 0.8, 1), sample_rate);
                let error = (measured - f64::from(hz)).abs();
                assert!(
                    error < 1e-3,
                    "{hz} Hz at {sample_rate} Hz measured {measured} (off {error})"
                );
            }
        }
    }

    #[test]
    fn no_step_exceeds_the_steepest_slope_a_continuous_sine_can_have() {
        let (sample_rate, hz, amplitude) = (48_000, 1_000.0_f32, 0.9_f32);
        let out = render(sample_rate, hz, amplitude, 1);
        let bound = TAU * f64::from(hz) / f64::from(sample_rate) * f64::from(amplitude)
            + 1.0 / (RAMP_SECONDS * f64::from(sample_rate))
            + 1e-6;
        let steepest = out
            .windows(2)
            .map(|w| f64::from((w[1] - w[0]).abs()))
            .fold(0.0, f64::max);
        assert!(steepest <= bound, "step of {steepest} exceeds {bound}");
    }

    #[test]
    fn the_level_ramps_in_and_out_without_overshoot() {
        let mut tone = SineOscillator::new(48_000);
        tone.set_frequency(440.0);
        tone.set_amplitude(0.5);
        let mut out = vec![0.0; 4_800];
        tone.fill(&mut out);
        assert!(out.iter().all(|s| s.abs() <= 0.5));
        tone.set_amplitude(0.0);
        tone.fill(&mut out);
        assert!(
            out[out.len() - 1_000..].iter().all(|&s| s == 0.0),
            "tone did not fade to silence"
        );
    }

    #[test]
    fn hostile_parameters_are_clamped_not_propagated() {
        let mut tone = SineOscillator::new(48_000);
        tone.set_frequency(f32::NAN);
        assert_eq!(tone.frequency().to_bits(), 0.0_f32.to_bits());
        tone.set_frequency(1e9);
        assert_eq!(tone.frequency().to_bits(), 24_000.0_f32.to_bits());
        tone.set_amplitude(f32::INFINITY);
        assert_eq!(tone.amplitude().to_bits(), 0.0_f32.to_bits());
        tone.set_amplitude(3.0);
        assert_eq!(tone.amplitude().to_bits(), 1.0_f32.to_bits());
    }

    proptest! {
        /// Phase continuity, stated as an invariant: any way of cutting a render into blocks gives
        /// bit-identical output.
        #[test]
        fn output_is_independent_of_block_boundaries(
            blocks in prop::collection::vec(1_usize..600, 1..40),
            hz in 20.0_f32..20_000.0,
        ) {
            let total: usize = blocks.iter().sum();
            let mut whole = SineOscillator::new(48_000);
            whole.set_frequency(hz);
            whole.set_amplitude(0.7);
            let mut expected = vec![0.0; total];
            whole.fill(&mut expected);

            let mut split = SineOscillator::new(48_000);
            split.set_frequency(hz);
            split.set_amplitude(0.7);
            let mut actual = vec![0.0; total];
            let mut start = 0;
            for size in blocks {
                split.fill(&mut actual[start..start + size]);
                start += size;
            }
            assert_eq!(actual, expected);
        }
    }
}
