//! The octave guard on estimates (`docs/AUDIO_ENGINE.md` §3, `docs/adr/0029-octave-guard.md`).
//!
//! Two defences, which are one mechanism here:
//!
//! - **(b) Jump hysteresis.** An estimate more than [`JUMP_CENTS`] from a *locked* one (accepted,
//!   within [`SAME_NOTE_CENTS`] of itself, for a window's worth of frames) is shown only once
//!   [`CONFIRM_FRAMES`] consecutive frames agree on it. Until then the frame gives `None`: never the
//!   old note, because a held note that the player has left is the commonest wrong reading.
//! - **(a) The τ/2 and 2τ test.** While such a jump waits, if the frame's NSDF has a peak of
//!   comparable clarity ([`COMPARABLE`]) at half or twice the chosen lag, and that peak is the
//!   held note, the frame shows it, flagged [`Octave::Ambiguous`]: the frame alone cannot tell the
//!   two octaves apart, and the history says which one was playing. It shows the letter and the
//!   cents, which an octave error keeps, without the octave number (`docs/PRODUCT_SPEC.md` §2).
//!
//! So an octave flicker of one or two frames inside a held note shows the held note, ambiguous,
//! and a real change costs at most two hops (5 ms in Mid and High) of `None` or ambiguity.
//! Why a jump the frame confirms three times is taken even against the τ/2 or 2τ peak: every
//! periodic signal is also periodic at 2τ, so the 2τ peak of a note an octave *up* is always
//! comparable. Preferring the old octave without a bound would keep a leap from E2 to E3 on E2
//! for good (`tests/chain.rs`, the no-gap leap sweep, holds it to zero wrong frames).
//!
//! Range-level octaves, for notes a range cannot test because `f/2` is beyond its longest lag,
//! are the [`crate::range::RangeSelector`]'s: `crate::chain::Analysis` combines the two.
//!
//! Pure state, allocation-free and panic-free: a few numbers per frame, no clock but the frames.

use crate::pitch::PitchEstimate;

/// Whether a frame's octave is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Octave {
    /// The estimate's octave is the note's.
    Known,
    /// The pitch class and the cents are the note's, but its octave is not yet decided: show the
    /// letter and the cents, not the octave number (`docs/PRODUCT_SPEC.md` §2, `ambiguous octave`).
    Ambiguous,
}

/// Larger jumps than this, in cents, from a locked estimate need [`CONFIRM_FRAMES`] frames: half an
/// octave, so every octave error and every fifth below or above counts, and no glide does.
pub const JUMP_CENTS: f64 = 600.0;

/// Consecutive frames that must agree, within [`SAME_NOTE_CENTS`], before a jump is shown.
pub const CONFIRM_FRAMES: usize = 3;

/// How close two estimates must be, in cents, to be the same note: half a semitone.
pub const SAME_NOTE_CENTS: f64 = 50.0;

/// A peak at τ/2 or 2τ is *comparable* when its clarity is at least this fraction of the chosen
/// peak's. Mirrors `Mpm`'s `k = 0.9`: a peak `Mpm` would have taken as the period had it come first
/// (`docs/AUDIO_ENGINE.md` §3, step 4). Measured (`pitch::tests`,
/// `peak_near_reads_the_octaves_of_the_frame_just_analysed`): the octave below a steady note is as
/// clear as the note, at least 0.999 of it, so it is always comparable; a weak fundamental's second
/// partial reads 0.55–0.65 of the note, which is not. The threshold's edge is `octave::tests`.
pub const COMPARABLE: f32 = 0.9;

/// An estimate whose octave is ambiguous because its range cannot see the octave below
/// (`crate::range::RangeTable::tests_octave`) is taken only from this clarity up: below it, the
/// frame has no estimate, for the guard, the selector and the display alike (a higher clarity
/// floor, `docs/AUDIO_ENGINE.md` §3 step 6, where the octave cannot be tested). Such an estimate is a partial of a fundamental the range cannot see, and the
/// other partials skew its NSDF peak, the more the lower its clarity: measured in `T-003c`, Mid's
/// reading of a weak bass E's second partial at clarity 0.60–0.64 is off by up to ±32 ¢, and in a
/// chord (A2 over a bass E) at 0.62–0.66 by ±88 ¢, a third note's pitch class.
pub const UNVERIFIED_CLARITY: f32 = 0.75;

/// What the guard made of one frame's estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guarded {
    /// The estimate to show.
    pub estimate: PitchEstimate,
    /// Its octave is [`Octave::Ambiguous`]: it is the held note's peak at τ/2 or 2τ, shown while a
    /// jump of the chosen one waits, or the held note again within a window of such a frame.
    pub ambiguous: bool,
}

/// The estimate-level octave guard: (a) and (b) above, over the frames of the selected range.
///
/// ```
/// use diapason_dsp::octave::OctaveGuard;
/// use diapason_dsp::pitch::PitchEstimate;
///
/// let at = |hz: f64| PitchEstimate { frequency_hz: hz, period_samples: 12_000.0 / hz, clarity: 0.99 };
/// let mut guard = OctaveGuard::new();
/// for _ in 0..16 {
///     guard.judge(at(110.0), None, None, 16); // locked after a window's worth of frames
/// }
/// // A fifth up waits for three frames that agree; meanwhile there is nothing to show.
/// assert_eq!(guard.judge(at(164.8), None, None, 16), None);
/// assert_eq!(guard.judge(at(164.8), None, None, 16), None);
/// assert!(guard.judge(at(164.8), None, None, 16).is_some());
/// ```
#[derive(Debug, Clone, Default)]
pub struct OctaveGuard {
    /// The latest accepted estimate, in hertz.
    held: Option<f64>,
    /// Consecutive frames it has been accepted within [`SAME_NOTE_CENTS`] of itself.
    run: usize,
    /// Consecutive frames without an estimate.
    misses: usize,
    /// A jump waiting: its latest estimate, and how many consecutive frames agreed on it.
    pending: Option<(f64, usize)>,
    /// Frames of the held note still shown ambiguous after the last one the frame alone could not
    /// decide (a window's worth): a pick flipping between two octaves every other frame, as under
    /// a lower string still ringing, keeps the octave ambiguous rather than blinking it.
    doubt: usize,
}

impl OctaveGuard {
    /// A guard with nothing held.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything: silence, or a range switch (its frames are a different detector's).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The latest accepted estimate, in hertz.
    #[must_use]
    pub fn held(&self) -> Option<f64> {
        self.held
    }

    /// A frame without an estimate. It breaks a run of confirming frames; `lock_frames` of them in
    /// a row and nothing is held any more.
    pub fn miss(&mut self, lock_frames: usize) {
        self.pending = None;
        self.misses += 1;
        if self.misses >= lock_frames {
            self.held = None;
            self.run = 0;
        }
    }

    /// Judge `pick`, `Mpm`'s estimate, with `up` and `down`, its NSDF's peaks near half and twice the
    /// chosen lag (`Mpm::peak_near`, `None` where there is none). `lock_frames` frames accepted in a
    /// row lock an estimate: the current range's window, `RangeSelector::held_frames`. Returns what
    /// to show, or `None` while a jump waits for confirmation.
    pub fn judge(
        &mut self,
        pick: PitchEstimate,
        up: Option<PitchEstimate>,
        down: Option<PitchEstimate>,
        lock_frames: usize,
    ) -> Option<Guarded> {
        self.misses = 0;
        let hz = pick.frequency_hz;
        let Some(held) = self.held.filter(|&held| cents(hz, held).abs() > JUMP_CENTS) else {
            return Some(self.accept(pick));
        };
        let agreeing = match self.pending {
            Some((previous, count)) if cents(hz, previous).abs() <= SAME_NOTE_CENTS => count + 1,
            _ => 1,
        };
        if agreeing >= CONFIRM_FRAMES {
            return Some(self.accept(pick));
        }
        // (a): the held note's octave, if the frame has a comparable peak there.
        let comparable = |candidate: &PitchEstimate| {
            candidate.clarity >= COMPARABLE * pick.clarity
                && cents(candidate.frequency_hz, held).abs() <= SAME_NOTE_CENTS
        };
        if let Some(bridge) = [up, down].into_iter().flatten().find(comparable) {
            self.pending = Some((hz, agreeing));
            self.doubt = lock_frames;
            return Some(Guarded {
                estimate: bridge,
                ambiguous: true,
            });
        }
        // (b): from an estimate not yet locked, a jump is taken at once (an onset settling).
        if self.run < lock_frames {
            return Some(self.accept(pick));
        }
        self.pending = Some((hz, agreeing));
        None
    }

    fn accept(&mut self, pick: PitchEstimate) -> Guarded {
        let hz = pick.frequency_hz;
        let same = self
            .held
            .is_some_and(|held| cents(hz, held).abs() <= SAME_NOTE_CENTS);
        self.run = if same { self.run.saturating_add(1) } else { 1 };
        let doubted = same && self.doubt > 0;
        self.doubt = if doubted { self.doubt - 1 } else { 0 };
        self.held = Some(hz);
        self.pending = None;
        Guarded {
            estimate: pick,
            ambiguous: doubted,
        }
    }
}

/// `1200 · log₂(a / b)`.
fn cents(a: f64, b: f64) -> f64 {
    1_200.0 * (a / b).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hz: f64, clarity: f32) -> PitchEstimate {
        PitchEstimate {
            frequency_hz: hz,
            period_samples: 12_000.0 / hz,
            clarity,
        }
    }

    /// A guard holding 82.41 Hz, locked (16 frames, Mid's window).
    fn locked() -> OctaveGuard {
        let mut guard = OctaveGuard::new();
        for _ in 0..16 {
            guard.judge(at(82.41, 0.99), None, None, 16);
        }
        guard
    }

    #[test]
    fn a_jump_from_a_locked_estimate_holds_for_two_frames_and_gives_way_on_the_third() {
        // Holds: two frames of a fifth up, then the held note again, show nothing wrong.
        let mut guard = locked();
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        let back = guard.judge(at(82.4, 0.99), None, None, 16).unwrap();
        assert!(!back.ambiguous && (back.estimate.frequency_hz - 82.4).abs() < 1e-9);
        // Gives way: three in a row.
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        assert_eq!(guard.judge(at(123.6, 0.95), None, None, 16), None);
        let jumped = guard.judge(at(123.5, 0.95), None, None, 16).unwrap();
        assert!((jumped.estimate.frequency_hz - 123.5).abs() < 1e-12);
        assert!(guard.held().is_some_and(|hz| (hz - 123.5).abs() < 1e-12));
        // A missing frame breaks the run; disagreeing frames restart it.
        let mut guard = locked();
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        guard.miss(16);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        assert_eq!(guard.judge(at(196.0, 0.95), None, None, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 16), None);
        assert!(guard.judge(at(123.5, 0.95), None, None, 16).is_some());
    }

    #[test]
    fn an_estimate_not_yet_locked_and_a_glide_move_at_once() {
        let mut guard = OctaveGuard::new();
        for _ in 0..15 {
            guard.judge(at(82.41, 0.99), None, None, 16);
        }
        assert!(guard.judge(at(164.8, 0.99), None, None, 16).is_some());
        // A bend, 590 ¢ in 59 steps of 10 ¢, never jumps.
        let mut guard = locked();
        for step in 1..=59 {
            let hz = 82.41 * 2_f64.powf(f64::from(step) * 10.0 / 1_200.0);
            assert!(
                guard.judge(at(hz, 0.99), None, None, 16).is_some(),
                "{step}"
            );
        }
        // Nothing held after a window of misses: the next note is taken at once.
        let mut guard = locked();
        for _ in 0..16 {
            guard.miss(16);
        }
        assert!(guard.judge(at(196.0, 0.99), None, None, 16).is_some());
    }

    #[test]
    fn a_comparable_peak_at_the_held_octave_is_shown_ambiguous_at_the_comparable_edge() {
        let pick = at(164.82, 0.98);
        // 2τ, the octave below, is the held note.
        let edge = at(82.41, COMPARABLE * 0.98);
        let mut guard = locked();
        let shown = guard.judge(pick, None, Some(edge), 16).unwrap();
        assert!(shown.ambiguous && shown.estimate == edge);
        // Just below the threshold it is not comparable: nothing is shown.
        let below = at(82.41, COMPARABLE * 0.98 - 1e-4);
        let mut guard = locked();
        assert_eq!(guard.judge(pick, None, Some(below), 16), None);
        // τ/2, the octave above, likewise, when the held note is above.
        let mut guard = OctaveGuard::new();
        for _ in 0..16 {
            guard.judge(at(329.63, 0.99), None, None, 16);
        }
        let up = at(329.6, 0.97);
        let shown = guard.judge(at(164.8, 0.99), Some(up), None, 16).unwrap();
        assert!(shown.ambiguous && shown.estimate == up);
        // A comparable peak that is not the held note changes nothing.
        let mut guard = locked();
        assert_eq!(guard.judge(pick, Some(at(329.6, 0.99)), None, 16), None);
    }

    #[test]
    fn a_pick_flipping_between_octaves_keeps_the_octave_ambiguous_for_a_window() {
        // A lower string ringing under the octave above: the pick flips every frame.
        let mut guard = locked();
        let (e2, e3) = (at(82.41, 1.0), at(164.82, 1.0));
        for _ in 0..40 {
            assert!(guard.judge(e3, None, Some(e2), 16).unwrap().ambiguous);
            assert!(guard.judge(e2, Some(e3), None, 16).unwrap().ambiguous);
        }
        // Once the upper one is gone, a window of agreeing frames makes it known again.
        for frame in 1..16 {
            assert!(
                guard.judge(e2, None, None, 16).unwrap().ambiguous,
                "{frame}"
            );
        }
        assert!(!guard.judge(e2, None, None, 16).unwrap().ambiguous);
    }

    #[test]
    fn the_bridge_lasts_only_while_the_jump_waits() {
        // A leap from E2 to E3: E3's frame is periodic at 2τ too, so 2τ is always comparable. The
        // held E2 is shown, ambiguous, for two frames; the third takes E3.
        let mut guard = locked();
        let e3 = at(164.82, 0.999);
        let e2 = at(82.41, 0.999);
        for _ in 0..2 {
            assert!(guard.judge(e3, None, Some(e2), 16).unwrap().ambiguous);
        }
        let third = guard.judge(e3, None, Some(e2), 16).unwrap();
        assert!(!third.ambiguous && third.estimate == e3);
        assert!(!guard.judge(e3, None, Some(e2), 16).unwrap().ambiguous);
    }
}
