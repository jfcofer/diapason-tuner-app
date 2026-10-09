//! Input level, measured over fixed windows.

use crate::convert::to_f32;

/// Root-mean-square level over consecutive, fixed-length windows.
///
/// A window is a count of samples, not of calls, so the reading depends only on the signal and
/// never on how a backend happened to cut it into blocks. Squares are summed in `f64`, which keeps a
/// full-scale window of any supported length exact to well below `f32` resolution.
/// Allocation-free and panic-free on every path.
///
/// ```
/// use diapason_dsp::level::RmsMeter;
///
/// let mut meter = RmsMeter::new(4);
/// meter.process(&[0.5, -0.5, 0.5]);
/// assert_eq!(meter.last().to_bits(), 0.0_f32.to_bits()); // the first window is not complete yet
/// meter.process(&[-0.5]);
/// assert_eq!(meter.last(), 0.5);
/// ```
#[derive(Debug, Clone)]
pub struct RmsMeter {
    window: u32,
    count: u32,
    sum_of_squares: f64,
    last: f32,
}

impl RmsMeter {
    /// A meter reporting once per `window` samples. A zero window is treated as one sample.
    #[must_use]
    pub fn new(window: u32) -> Self {
        Self {
            window: window.max(1),
            count: 0,
            sum_of_squares: 0.0,
            last: 0.0,
        }
    }

    /// Samples per reading.
    #[must_use]
    pub fn window(&self) -> u32 {
        self.window
    }

    /// Add one sample.
    #[inline]
    pub fn process_sample(&mut self, sample: f32) {
        let sample = f64::from(sample);
        self.sum_of_squares += sample * sample;
        self.count += 1;
        if self.count == self.window {
            self.last = to_f32((self.sum_of_squares / f64::from(self.window)).sqrt());
            self.sum_of_squares = 0.0;
            self.count = 0;
        }
    }

    /// Add every sample in `samples`, in order.
    pub fn process(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.process_sample(sample);
        }
    }

    /// The RMS of the most recently completed window; zero before the first one completes.
    #[must_use]
    pub fn last(&self) -> f32 {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::f64::consts::{FRAC_1_SQRT_2, TAU};

    /// A closed-form sine fixture, independent of the oscillator in this crate.
    fn sine(amplitude: f64, hz: f64, sample_rate: u32, samples: u32) -> Vec<f32> {
        (0..samples)
            .map(|i| to_f32(amplitude * (TAU * hz * f64::from(i) / f64::from(sample_rate)).sin()))
            .collect()
    }

    fn relative_error(measured: f32, expected: f64) -> f64 {
        ((f64::from(measured) - expected) / expected).abs()
    }

    #[test]
    fn a_sine_over_whole_periods_reads_amplitude_over_root_two() {
        // 2400 samples is 50 ms at 48 kHz; each frequency fits a whole number of periods in it.
        for (amplitude, hz) in [(1.0, 1_000.0), (0.5, 100.0), (0.01, 2_000.0)] {
            let mut meter = RmsMeter::new(2_400);
            meter.process(&sine(amplitude, hz, 48_000, 4_800));
            let error = relative_error(meter.last(), amplitude * FRAC_1_SQRT_2);
            assert!(
                error < 1e-6,
                "amplitude {amplitude} at {hz} Hz: relative error {error}"
            );
        }
    }

    #[test]
    fn silence_reads_zero_and_a_square_wave_reads_its_amplitude() {
        let mut meter = RmsMeter::new(480);
        meter.process(&[0.0; 960]);
        assert_eq!(meter.last().to_bits(), 0.0_f32.to_bits());
        let square: Vec<f32> = (0..960)
            .map(|i| if i % 2 == 0 { 0.25 } else { -0.25 })
            .collect();
        meter.process(&square);
        assert_eq!(meter.last().to_bits(), 0.25_f32.to_bits());
    }

    #[test]
    fn a_zero_window_is_one_sample() {
        let mut meter = RmsMeter::new(0);
        meter.process_sample(-0.75);
        assert_eq!((meter.window(), meter.last()), (1, 0.75));
    }

    proptest! {
        /// Readings depend on the signal only, never on how it was cut into blocks.
        #[test]
        fn readings_are_independent_of_block_boundaries(
            blocks in prop::collection::vec(1_usize..600, 1..40),
            window in 1_u32..2_000,
        ) {
            let total: usize = blocks.iter().sum();
            let signal = sine(0.6, 330.0, 48_000, u32::try_from(total).unwrap_or(u32::MAX));
            let mut whole = RmsMeter::new(window);
            whole.process(&signal);

            let mut split = RmsMeter::new(window);
            let mut start = 0;
            for size in blocks {
                split.process(&signal[start..start + size]);
                start += size;
            }
            assert_eq!(split.last().to_bits(), whole.last().to_bits());
            assert_eq!(split.count, whole.count);
        }
    }
}
