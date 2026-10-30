//! Lock detection: the "held in tune" indicator (`docs/AUDIO_ENGINE.md` §3, `docs/adr/0032`).
//!
//! A note locks once its cents have stayed within the tolerance for [`LOCK_SECONDS`] of
//! consecutive frames with clarity above [`LOCK_CLARITY`], and unlocks only beyond the tolerance
//! plus [`UNLOCK_MARGIN_CENTS`], so a note sitting at the edge does not make the indicator chatter.
//! Time is the frames' positions on the audio clock, never a timer.
//!
//! It reads `frequency_raw`, as cents from the target note, which the caller computes: the target
//! depends on the reference A4, the temperament, the transposition and per-string offsets (§4),
//! which belong to the note table and the session, not to `dsp`. An `ambiguous octave` frame keeps
//! its cents, so it locks like any other.
//!
//! Pure state, allocation-free and panic-free.

use crate::convert::index_from_f64;

/// How long, in seconds, the cents must stay within the tolerance before the note locks.
pub const LOCK_SECONDS: f64 = 0.25;

/// A frame counts towards a lock only above this clarity.
pub const LOCK_CLARITY: f32 = 0.8;

/// How far past the tolerance, in cents, a locked note may go before it unlocks.
pub const UNLOCK_MARGIN_CENTS: f64 = 1.5;

/// The tolerance, in cents, unless the player sets another (`docs/PRODUCT_SPEC.md`).
pub const DEFAULT_TOLERANCE_CENTS: f64 = 3.0;

/// The narrowest tolerance the player may set, in cents.
pub const MIN_TOLERANCE_CENTS: f64 = 1.0;

/// The widest tolerance the player may set, in cents.
pub const MAX_TOLERANCE_CENTS: f64 = 10.0;

/// The lock indicator's state.
///
/// ```
/// use diapason_dsp::lock::Lock;
///
/// let mut lock = Lock::new(48_000, 3.0);
/// let hop = 120; // 2.5 ms
/// let mut locked_at = None;
/// for k in 1..=200_u64 {
///     if lock.observe(Some(1.0), 0.95, k * hop) && locked_at.is_none() {
///         locked_at = Some(k);
///     }
/// }
/// // The first frame starts the run; 250 ms (100 hops) later the note locks.
/// assert_eq!(locked_at, Some(101));
/// assert!(!lock.observe(Some(4.6), 0.95, 201 * hop)); // past 3 + 1.5 ¢
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Lock {
    tolerance: f64,
    hold: u64,
    /// Where the current run of frames within the tolerance began.
    since: Option<u64>,
    locked: bool,
}

impl Lock {
    /// A lock for frames whose positions count samples at `sample_rate`, with the tolerance in
    /// cents clamped to [`MIN_TOLERANCE_CENTS`]–[`MAX_TOLERANCE_CENTS`].
    #[must_use]
    pub fn new(sample_rate: u32, tolerance_cents: f64) -> Self {
        let hold = index_from_f64((LOCK_SECONDS * f64::from(sample_rate)).round());
        let mut lock = Self {
            tolerance: DEFAULT_TOLERANCE_CENTS,
            hold: u64::try_from(hold).unwrap_or(u64::MAX),
            since: None,
            locked: false,
        };
        lock.set_tolerance(tolerance_cents);
        lock
    }

    /// Set the tolerance in cents, clamped to [`MIN_TOLERANCE_CENTS`]–[`MAX_TOLERANCE_CENTS`]. A
    /// value that is not a number keeps the current one.
    pub fn set_tolerance(&mut self, cents: f64) {
        if !cents.is_nan() {
            self.tolerance = cents.clamp(MIN_TOLERANCE_CENTS, MAX_TOLERANCE_CENTS);
        }
    }

    /// The tolerance in use, in cents.
    #[must_use]
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// Whether the note is locked.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Forget the note: the gate closed, or the target changed.
    pub fn reset(&mut self) {
        self.since = None;
        self.locked = false;
    }

    /// Take one frame: its cents from the target (`None` without an estimate), its clarity, and its
    /// end on the audio clock. Returns whether the note is locked after it.
    pub fn observe(&mut self, cents: Option<f64>, clarity: f32, end: u64) -> bool {
        let clear = clarity > LOCK_CLARITY;
        let off = cents.filter(|c| c.is_finite() && clear).map(f64::abs);
        if self.locked {
            if off.is_some_and(|off| off <= self.tolerance + UNLOCK_MARGIN_CENTS) {
                return true;
            }
            self.reset();
        }
        if off.is_some_and(|off| off <= self.tolerance) {
            let since = *self.since.get_or_insert(end);
            self.locked = end.saturating_sub(since) >= self.hold;
        } else {
            self.since = None;
        }
        self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    /// Mid's hop at 48 kHz: 2.5 ms.
    const HOP: u64 = 120;
    /// Frames in [`LOCK_SECONDS`] at [`HOP`].
    const HOLD_FRAMES: u64 = 100;

    /// Feed `cents` at one frame per hop from `from`; the frames after which the note is locked.
    fn run(lock: &mut Lock, cents: impl IntoIterator<Item = f64>, from: u64) -> Vec<bool> {
        cents
            .into_iter()
            .zip(from..)
            .map(|(c, k)| lock.observe(Some(c), 0.95, k * HOP))
            .collect()
    }

    #[test]
    fn a_note_within_the_tolerance_locks_after_250_ms() {
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        let locked = run(&mut lock, std::iter::repeat_n(-2.9, 150), 1);
        let first = locked.iter().position(|&l| l).unwrap();
        assert_eq!(u64::try_from(first).unwrap(), HOLD_FRAMES);
        assert!(locked[first..].iter().all(|&l| l));
    }

    #[test]
    fn no_lock_outside_the_tolerance_unclear_or_interrupted() {
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        assert!(!run(&mut lock, std::iter::repeat_n(3.1, 400), 1).contains(&true));
        // Clarity must exceed 0.8.
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        assert!((1..400).all(|k| !lock.observe(Some(0.0), LOCK_CLARITY, k * HOP)));
        // One frame out, or one without an estimate, every 99 frames: the run never reaches 250 ms.
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        let cents = (0..400).map(|k| if k % 99 == 98 { 3.5 } else { 0.0 });
        assert!(!run(&mut lock, cents, 1).contains(&true));
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        assert!((1..400_u64).all(|k| {
            let cents = (k % 99 != 98).then_some(0.0);
            !lock.observe(cents, 0.95, k * HOP)
        }));
    }

    #[test]
    fn a_note_at_the_edge_does_not_chatter() {
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        run(&mut lock, std::iter::repeat_n(0.0, 101), 1);
        assert!(lock.is_locked());
        // Wandering across the tolerance, inside the wider band: stays locked on every frame.
        let edge = (0..400).map(|k| if k % 2 == 0 { 2.8 } else { 4.4 });
        assert!(run(&mut lock, edge, 102).iter().all(|&l| l));
        // Past it, unlocked; back at the edge, it stays unlocked until 250 ms within the tolerance.
        assert!(!lock.observe(Some(-4.6), 0.95, 502 * HOP));
        let edge = (0..400).map(|k| if k % 2 == 0 { 2.8 } else { 4.4 });
        assert!(!run(&mut lock, edge, 503).contains(&true));
        let back = run(&mut lock, std::iter::repeat_n(2.8, 101), 903);
        assert_eq!(back.iter().position(|&l| l), Some(100));
    }

    #[test]
    fn time_is_the_audio_clock_whatever_the_hop() {
        // Frames 5 ms apart (Low), then 2.5 ms (Mid): 250 ms of positions, not 100 frames.
        let mut lock = Lock::new(RATE, DEFAULT_TOLERANCE_CENTS);
        let ends = (0..30)
            .map(|k| 240 * k)
            .chain((1..=100).map(|k| 240 * 29 + 120 * k));
        let locked_at = ends
            .map(|end| (end, lock.observe(Some(0.0), 0.95, end)))
            .find(|&(_, locked)| locked)
            .map(|(end, _)| end);
        assert_eq!(locked_at, Some(12_000));
    }

    #[test]
    fn the_tolerance_is_clamped_and_resets_forget_the_note() {
        let mut lock = Lock::new(RATE, 0.2);
        assert!((lock.tolerance() - MIN_TOLERANCE_CENTS).abs() < f64::EPSILON);
        lock.set_tolerance(25.0);
        assert!((lock.tolerance() - MAX_TOLERANCE_CENTS).abs() < f64::EPSILON);
        lock.set_tolerance(f64::NAN);
        assert!((lock.tolerance() - MAX_TOLERANCE_CENTS).abs() < f64::EPSILON);
        run(&mut lock, std::iter::repeat_n(9.0, 101), 1);
        assert!(lock.is_locked());
        lock.reset();
        assert!(!lock.is_locked());
        assert!(!lock.observe(Some(f64::NAN), 0.95, 200 * HOP));
    }
}
