//! What the needle shows: `frequency_display`, the octave-guarded estimate smoothed
//! (`docs/AUDIO_ENGINE.md` §3, `docs/adr/0032-display-smoothing-and-lock.md`).
//!
//! The estimate itself is `frequency_raw`, unsmoothed, for lock detection and the strobe. The
//! display is a median of five, which removes single frames that miss, then a one-euro filter
//! (Casiez, Roussel and Vogel, "1€ Filter", CHI 2012), which holds the needle still at rest and
//! lets it follow a turning peg. [`Needle`] composes them so that smoothing never undoes the
//! octave guard (`crate::octave`):
//! - frames of two notes are never mixed: an estimate whose pitch class is more than
//!   [`SAME_NOTE_CENTS`] from the history's gathers apart, and becomes the note once
//!   [`WARM_UP_FRAMES`] of it are in, so one frame that misses does not cost the held note;
//! - the same pitch class in another octave (the guard settling a held note's octave, known or
//!   `ambiguous`) is the same note: the history moves by that octave, and the needle neither
//!   blanks nor jumps in cents. Its values never mix two octaves;
//! - a frame without the held note shows none, never the old note, as the guard's own frames do.
//!   The history outlives at most [`HELD_GAP_FRAMES`] of them, as long as a guard's confirmation;
//!   after a longer gap a note starts again;
//! - after a reset nothing is shown until [`WARM_UP_FRAMES`] frames of the new note are in, so the
//!   first value shown is already a median. That is [`SMOOTHING_HOPS`], the latency ledger's line;
//! - the needle never trails the median by more than [`MAX_LAG_CENTS`]. An onset does not step: the
//!   estimate converges as the window fills, from as far as 45 ¢ off, and a median of five follows
//!   that ramp two frames behind. The one-euro filter at its rest cutoff trails it further, up to
//!   19 ms past the ledger's line (Low, a low B). With the bound the needle is within ±5 ¢ once the
//!   median is within 2 ¢, and the filter alone acts on a held note, which moves far less. The
//!   ledger holds as it is built, worst case against worst case (`tests/chain.rs`,
//!   `onset_latency_is_within_the_ledger`), not run by run: a run whose estimate settles early may
//!   see its needle up to 30 ms after it (`docs/adr/0032`).
//!
//! The one-euro filter runs in cents, not hertz: its `beta` scales the cutoff with the speed of the
//! signal, and a speed in hertz per second means 45 times more at 1400 Hz than at 31 Hz. In cents
//! it means the same across the range. Its time step comes from the frames' positions on the audio
//! clock, so a range switch, which changes the hop, needs nothing special.
//!
//! Pure state, allocation-free and panic-free: five numbers and a filter per frame.

use std::f64::consts::TAU;

use crate::convert::index_to_f64;
use crate::octave::SAME_NOTE_CENTS;
use crate::range::SMOOTHING_HOPS;

/// Estimates the median runs over (`docs/AUDIO_ENGINE.md` §3).
pub const MEDIAN_LEN: usize = 5;

/// Frames of a new note held before the first value is shown: the median of three follows a step
/// on the third new frame, two hops after the first ([`SMOOTHING_HOPS`]).
pub const WARM_UP_FRAMES: usize = SMOOTHING_HOPS + 1;

/// The one-euro filter's cutoff at rest, in hertz (`docs/AUDIO_ENGINE.md` §3): a time constant of
/// 265 ms on a held note.
pub const MIN_CUTOFF_HZ: f64 = 0.6;

/// How fast the cutoff rises with the needle's speed, in hertz per cent per second
/// (`docs/AUDIO_ENGINE.md` §3): a peg turned at 100 ¢/s is followed with a 2.6 Hz cutoff.
pub const BETA: f64 = 0.02;

/// The cutoff of the speed estimate the filter scales by, in hertz: the paper's default, which
/// §3 leaves unstated (`docs/adr/0032`).
pub const D_CUTOFF_HZ: f64 = 1.0;

/// Frames without the held note, no estimate or another note's, that the history outlives: the
/// median's own length. A guard's confirmation of a jump (`octave::CONFIRM_FRAMES`) is shorter; a
/// string stopped and played again is longer, and starts a new note.
pub const HELD_GAP_FRAMES: usize = MEDIAN_LEN;

/// The furthest the needle may trail the median, in cents (`docs/adr/0032`). Measured in `T-003f`
/// over `tests/chain.rs`'s onset grid and held sawtooth notes in noise: the needle meets the
/// ledger's smoothing line for any bound up to 4 ¢ (4.5 ¢ misses by 3 ms), and wobbles at most
/// 1 ¢ at 10 dB SNR for any bound from 3 ¢ (2 ¢: 2.8 ¢). The lower end keeps the latency margin.
pub const MAX_LAG_CENTS: f64 = 3.0;

/// The median of the last [`MEDIAN_LEN`] values pushed, or of all of them while fewer are held.
///
/// ```
/// use diapason_dsp::smooth::Median;
///
/// let mut median = Median::new();
/// for x in [1.0, 9.0, 2.0] {
///     median.push(x);
/// }
/// assert_eq!(median.value(), Some(2.0));
/// median.push(3.0);
/// assert_eq!(median.value(), Some(2.5)); // four held: the mean of the middle two
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Median {
    values: [f64; MEDIAN_LEN],
    /// Where the next value goes.
    next: usize,
    /// Values held, at most [`MEDIAN_LEN`].
    len: usize,
}

impl Median {
    /// An empty median.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget every value.
    pub fn clear(&mut self) {
        self.len = 0;
        self.next = 0;
    }

    /// Values held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no value is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Multiply every value held by `factor`.
    pub fn scale(&mut self, factor: f64) {
        for value in &mut self.values {
            *value *= factor;
        }
    }

    /// Add `value`, dropping the oldest once [`MEDIAN_LEN`] are held.
    pub fn push(&mut self, value: f64) {
        if let Some(slot) = self.values.get_mut(self.next) {
            *slot = value;
        }
        self.next = (self.next + 1) % MEDIAN_LEN;
        self.len = (self.len + 1).min(MEDIAN_LEN);
    }

    /// The median of the values held: the middle one, or the mean of the middle two. `None` when
    /// empty.
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        let mut sorted = self.values;
        let held = sorted.get_mut(..self.len)?;
        held.sort_unstable_by(f64::total_cmp);
        let upper = *held.get(self.len / 2)?;
        if self.len % 2 == 1 {
            Some(upper)
        } else {
            Some(f64::midpoint(*held.get(self.len / 2 - 1)?, upper))
        }
    }
}

/// The one-euro filter on one value sampled at irregular times.
///
/// ```
/// use diapason_dsp::smooth::OneEuro;
///
/// let mut filter = OneEuro::new();
/// assert_eq!(filter.filter(10.0, 0.005), 10.0); // the first value passes unchanged
/// let next = filter.filter(11.0, 0.005);
/// assert!(next > 10.0 && next < 11.0);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct OneEuro {
    /// The last output and the last speed estimate, once a value has been filtered.
    state: Option<(f64, f64)>,
}

impl OneEuro {
    /// A filter that passes its first value unchanged.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the past: the next value passes unchanged.
    pub fn reset(&mut self) {
        self.state = None;
    }

    /// Filter `x`, sampled `dt` seconds after the previous value. A `dt` that is not positive
    /// (two values at one instant) leaves the output where it was.
    pub fn filter(&mut self, x: f64, dt: f64) -> f64 {
        let Some((previous, speed)) = self.state else {
            self.state = Some((x, 0.0));
            return x;
        };
        if dt <= 0.0 || !dt.is_finite() {
            return previous;
        }
        let speed = lerp(alpha(D_CUTOFF_HZ, dt), speed, (x - previous) / dt);
        let cutoff = MIN_CUTOFF_HZ + BETA * speed.abs();
        let out = lerp(alpha(cutoff, dt), previous, x);
        self.state = Some((out, speed));
        out
    }

    /// Move the last output to within `max_lag` of `x`, the value just filtered, keeping the speed
    /// estimate: the output follows a ramp no further behind than that, with no jump.
    pub fn bound(&mut self, x: f64, max_lag: f64) -> Option<f64> {
        let (out, _) = self.state.as_mut()?;
        *out = out.clamp(x - max_lag, x + max_lag);
        Some(*out)
    }
}

/// The smoothing factor of a first-order low-pass at `cutoff` hertz for a step of `dt` seconds.
fn alpha(cutoff: f64, dt: f64) -> f64 {
    let tau = 1.0 / (TAU * cutoff);
    1.0 / (1.0 + tau / dt)
}

fn lerp(alpha: f64, from: f64, to: f64) -> f64 {
    from + alpha * (to - from)
}

/// `frequency_display`, from the octave guard's frames (see the module docs).
///
/// ```
/// use diapason_dsp::smooth::Needle;
///
/// let mut display = Needle::new(48_000);
/// assert_eq!(display.update(Some(110.0), 240), None); // warming up
/// assert_eq!(display.update(Some(110.2), 480), None);
/// let shown = display.update(Some(109.9), 720).expect("three frames of one note");
/// assert!((shown - 110.0).abs() < 1e-9); // their median
/// assert_eq!(display.update(None, 960), None); // no estimate: no note, never the old one
/// let octave = display.update(Some(220.0), 1_200).expect("the same note, an octave up");
/// assert!((octave - 220.0).abs() < 1e-9);
/// assert_eq!(display.update(Some(233.1), 1_440), None); // a new note warms up again
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Needle {
    sample_rate: f64,
    median: Median,
    filter: OneEuro,
    /// The note the filter runs relative to, in hertz: the first value it showed.
    origin: f64,
    /// The position of the last frame filtered.
    filtered_at: u64,
    /// Frames since the last one of the held note: without an estimate, or of another note.
    missed: usize,
    /// Frames of another note than the held one, gathering until they are [`WARM_UP_FRAMES`].
    fresh: Median,
}

impl Needle {
    /// An empty display for input at `sample_rate`, the rate of the frames' positions.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: f64::from(sample_rate.max(1)),
            median: Median::new(),
            filter: OneEuro::new(),
            origin: 0.0,
            filtered_at: 0,
            missed: 0,
            fresh: Median::new(),
        }
    }

    /// Forget the note: the gate closed, or the chain was reset.
    pub fn reset(&mut self) {
        self.median.clear();
        self.fresh.clear();
        self.filter.reset();
        self.missed = 0;
    }

    /// Take a frame's estimate, in hertz, and its end on the audio clock; return what the needle
    /// shows. `None` while there is no estimate and while a new note warms up.
    pub fn update(&mut self, hz: Option<f64>, end: u64) -> Option<f64> {
        let Some(hz) = hz.filter(|hz| hz.is_finite() && *hz > 0.0) else {
            self.missed += 1;
            return None;
        };
        let held = self
            .median
            .value()
            .filter(|_| self.missed <= HELD_GAP_FRAMES);
        if let Some(octaves) = held.and_then(|held| octaves_to(hz, held)) {
            // The held note, in this octave or another: no value of the old octave stays.
            if octaves != 0.0 {
                let factor = octaves.exp2();
                self.median.scale(factor);
                self.origin *= factor;
            }
            self.missed = 0;
            self.fresh.clear();
            self.median.push(hz);
        } else {
            // Another note: it gathers apart, so a single frame that misses leaves the held note
            // whole, and it takes over once it has the frames a new note waits for.
            self.missed += 1;
            let fresh = self.fresh.value().and_then(|fresh| octaves_to(hz, fresh));
            match fresh {
                Some(octaves) if octaves != 0.0 => self.fresh.scale(octaves.exp2()),
                Some(_) => {}
                None => self.fresh.clear(),
            }
            self.fresh.push(hz);
            if self.fresh.len() < WARM_UP_FRAMES {
                return None;
            }
            self.median = self.fresh;
            self.fresh.clear();
            self.filter.reset();
            self.missed = 0;
        }
        if self.median.len() < WARM_UP_FRAMES {
            return None;
        }
        let median = self.median.value()?;
        let dt = if self.filter_started() {
            let samples = usize::try_from(end.saturating_sub(self.filtered_at)).ok()?;
            index_to_f64(samples) / self.sample_rate
        } else {
            self.origin = median;
            0.0
        };
        self.filtered_at = end;
        let x = cents(median, self.origin);
        self.filter.filter(x, dt);
        let shown = self.filter.bound(x, MAX_LAG_CENTS)?;
        Some(self.origin * (shown / 1_200.0).exp2())
    }

    fn filter_started(&self) -> bool {
        self.filter.state.is_some()
    }
}

fn cents(a: f64, b: f64) -> f64 {
    1_200.0 * (a / b).log2()
}

/// Whole octaves from `held` to `hz` if `hz` is `held`'s pitch class, within [`SAME_NOTE_CENTS`].
fn octaves_to(hz: f64, held: f64) -> Option<f64> {
    let off = cents(hz, held);
    let octaves = (off / 1_200.0).round();
    ((off - 1_200.0 * octaves).abs() <= SAME_NOTE_CENTS).then_some(octaves)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    /// Mid's hop at 48 kHz, in input samples: 2.5 ms.
    const HOP: u64 = 120;

    fn feed(display: &mut Needle, values: &[Option<f64>], first_end: u64) -> Vec<Option<f64>> {
        values
            .iter()
            .zip(0..)
            .map(|(&hz, k)| display.update(hz, first_end + k * HOP))
            .collect()
    }

    #[test]
    fn the_median_is_the_middle_of_what_it_holds() {
        let mut median = Median::new();
        assert_eq!(median.value(), None);
        median.push(4.0);
        assert_eq!(median.value(), Some(4.0));
        median.push(2.0);
        assert_eq!(median.value(), Some(3.0));
        for x in [100.0, -50.0, 3.0] {
            median.push(x);
        }
        assert_eq!(median.len(), MEDIAN_LEN);
        assert_eq!(median.value(), Some(3.0));
        // The oldest (4.0) goes first: {2, 100, -50, 3, 1}.
        median.push(1.0);
        assert_eq!(median.value(), Some(2.0));
        median.clear();
        assert!(median.is_empty());
        assert_eq!(median.value(), None);
    }

    #[test]
    fn the_median_removes_one_frame_and_two_that_miss() {
        let mut median = Median::new();
        for x in [0.0, 8.0, 0.1, -7.0, -0.1] {
            median.push(x);
        }
        assert_eq!(median.value(), Some(0.0));
    }

    #[test]
    fn one_euro_is_still_at_rest_and_follows_a_ramp() {
        let dt = 0.005;
        // At rest, ±1 alternating, the speed estimate stays small and so does the cutoff:
        // the output's swing is a fraction of the input's.
        let mut filter = OneEuro::new();
        let mut swing: f64 = 0.0;
        for k in 0..400 {
            let x = if k % 2 == 0 { 1.0 } else { -1.0 };
            let y = filter.filter(x, dt);
            if k > 200 {
                swing = swing.max(y.abs());
            }
        }
        assert!(swing < 0.1, "swing {swing}");
        // A peg turned at 100 ¢/s. As in the paper's reference code, the speed is read from the
        // last output, so a lag `L` adds `L / dt` to it: the lag settles at the fixed point
        // `L = v · τ(MIN_CUTOFF_HZ + BETA · (v + L / dt))`, under 2 ¢ here.
        let speed = 100.0;
        let mut filter = OneEuro::new();
        let mut lag = 0.0;
        for k in 0..400 {
            let x = speed * dt * f64::from(k);
            lag = x - filter.filter(x, dt);
        }
        let cutoff = MIN_CUTOFF_HZ + BETA * (speed + lag / dt);
        let expected = speed / (TAU * cutoff);
        assert!(lag < 2.0, "lag {lag} ¢");
        assert!(
            (lag - expected).abs() < 0.05,
            "lag {lag} ¢, fixed point {expected} ¢"
        );
    }

    #[test]
    fn the_bound_keeps_the_output_within_reach_of_a_ramp() {
        let dt = 0.005;
        let mut filter = OneEuro::new();
        assert_eq!(
            filter.bound(0.0, MAX_LAG_CENTS),
            None,
            "nothing filtered yet"
        );
        // An onset converging at 2000 ¢/s: unbounded, the filter trails it by 24 ¢.
        let (mut unbounded, mut bounded) = (OneEuro::new(), OneEuro::new());
        let (mut lag, mut bounded_lag): (f64, f64) = (0.0, 0.0);
        for k in 0..10 {
            let x = -2_000.0 * dt * f64::from(k);
            lag = lag.max((unbounded.filter(x, dt) - x).abs());
            bounded.filter(x, dt);
            let y = bounded.bound(x, MAX_LAG_CENTS).unwrap();
            bounded_lag = bounded_lag.max((y - x).abs());
        }
        assert!(lag > 5.0 * MAX_LAG_CENTS, "{lag}");
        assert!(bounded_lag <= MAX_LAG_CENTS + 1e-12, "{bounded_lag}");
        // At rest the bound does nothing: the output is already within it.
        filter.filter(0.5, dt);
        let out = filter.filter(-0.5, dt);
        assert_eq!(
            out.to_bits(),
            filter.bound(-0.5, MAX_LAG_CENTS).unwrap().to_bits()
        );
    }

    #[test]
    fn one_euro_holds_on_a_step_of_no_time() {
        let mut filter = OneEuro::new();
        filter.filter(1.0, 0.0);
        assert_eq!(filter.filter(5.0, 0.0).to_bits(), 1.0_f64.to_bits());
        assert_eq!(filter.filter(5.0, f64::NAN).to_bits(), 1.0_f64.to_bits());
        filter.reset();
        assert_eq!(filter.filter(5.0, 0.0).to_bits(), 5.0_f64.to_bits());
    }

    #[test]
    fn a_new_note_warms_up_and_shows_the_median_first() {
        let mut display = Needle::new(RATE);
        let shown = feed(
            &mut display,
            &[Some(110.0), Some(111.0), Some(110.1), Some(110.05)],
            HOP,
        );
        assert_eq!(&shown[..2], &[None, None]);
        assert_eq!(shown[2], Some(110.1));
        let fourth = shown[3].unwrap();
        assert!(cents(fourth, 110.1).abs() < 1.0, "{fourth}");
    }

    #[test]
    fn notes_are_never_mixed() {
        let mut display = Needle::new(RATE);
        let a2 = [Some(110.0 * (2.0_f64 / 1_200.0).exp2()); 6];
        feed(&mut display, &a2, HOP);
        // The guard settling the octave: the same pitch class an octave up and down, shown at once,
        // in its own octave, with its cents (+2 ¢) unchanged. No value of the old octave mixes in.
        for (octave, from) in [(2.0, 7), (0.5, 8), (1.0, 9)] {
            let hz = 110.0 * octave * (2.0_f64 / 1_200.0).exp2();
            let shown = display.update(Some(hz), from * HOP).unwrap();
            assert!(cents(shown, hz).abs() < 1e-9, "{octave}: {shown}");
        }
        // A semitone starts over, and shows nothing until its own frames do; so does a tritone.
        for (hz, from) in [(116.54, 14), (164.81, 21)] {
            let shown = feed(&mut display, &[Some(hz); 3], from * HOP);
            assert_eq!(&shown[..2], &[None, None], "{hz}");
            assert!(cents(shown[2].unwrap(), hz).abs() < 1e-9, "{hz}");
        }
        // A wobble inside half a semitone is the same note: no warm-up.
        let shown = feed(
            &mut display,
            &[Some(164.81 * (40.0_f64 / 1_200.0).exp2())],
            28 * HOP,
        );
        assert!(shown[0].is_some());
    }

    #[test]
    fn one_frame_of_another_note_leaves_the_held_note_whole() {
        let mut display = Needle::new(RATE);
        feed(&mut display, &[Some(82.41); 5], HOP);
        // An attack's stray frame, 52 ¢ off: shown as nothing, and the held note goes on at once.
        let stray = 82.41 * (52.0_f64 / 1_200.0).exp2();
        let shown = feed(&mut display, &[Some(stray), Some(82.41)], 6 * HOP);
        assert_eq!(shown[0], None);
        assert!(cents(shown[1].unwrap(), 82.41).abs() < 1e-9, "{shown:?}");
        // Two stray frames of one note, then the held one: still whole.
        let shown = feed(
            &mut display,
            &[Some(stray), Some(stray), Some(82.41)],
            8 * HOP,
        );
        assert_eq!(&shown[..2], &[None, None]);
        assert!(shown[2].is_some());
        // Three of it are a new note, shown from its third frame.
        let shown = feed(&mut display, &[Some(stray); 3], 11 * HOP);
        assert_eq!(&shown[..2], &[None, None]);
        assert!(cents(shown[2].unwrap(), stray).abs() < 1e-9, "{shown:?}");
    }

    #[test]
    fn no_estimate_shows_no_note_and_the_same_note_resumes() {
        let mut display = Needle::new(RATE);
        feed(&mut display, &[Some(82.41); 5], HOP);
        let shown = feed(&mut display, &[None, Some(82.41)], 6 * HOP);
        assert_eq!(shown[0], None);
        assert!(
            shown[1].is_some(),
            "the same note resumes without a warm-up"
        );
        // A gap as long as a guard's confirmation keeps the note; a longer one starts again.
        let gap = [None; HELD_GAP_FRAMES];
        let shown = feed(&mut display, &[&gap[..], &[Some(82.41)]].concat(), 8 * HOP);
        assert!(shown[HELD_GAP_FRAMES].is_some());
        let gap = [None; HELD_GAP_FRAMES + 1];
        let shown = feed(&mut display, &[&gap[..], &[Some(82.41)]].concat(), 20 * HOP);
        assert_eq!(
            shown[HELD_GAP_FRAMES + 1],
            None,
            "a stale history is dropped"
        );
        display.reset();
        assert_eq!(display.update(Some(82.41), 40 * HOP), None);
    }

    #[test]
    fn nothing_that_is_not_a_frequency_is_shown() {
        let mut display = Needle::new(RATE);
        for hz in [f64::NAN, f64::INFINITY, 0.0, -110.0] {
            assert_eq!(display.update(Some(hz), HOP), None);
        }
        assert!(display.median.is_empty());
    }
}
