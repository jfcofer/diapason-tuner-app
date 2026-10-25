//! The octave guard on estimates (`docs/AUDIO_ENGINE.md` §3, `docs/adr/0029-octave-guard.md`).
//!
//! Two defences, which are one mechanism here:
//!
//! - **(b) Jump hysteresis.** An estimate more than [`JUMP_CENTS`] from a *locked* one (accepted,
//!   within [`SAME_NOTE_CENTS`] of itself, for a window's worth of frames) is shown only once
//!   [`CONFIRM_FRAMES`] consecutive frames agree on it. Until then the frame gives `None`: never the
//!   old note, because a held note that the player has left is the commonest wrong reading.
//! - **(a) The τ/2 and 2τ test.** When the estimate jumps from the held one, locked or not, and
//!   the frame's NSDF has a peak of comparable clarity ([`COMPARABLE`]) at half or twice the
//!   chosen lag that is the held note, the frame shows that peak, flagged [`Octave::Ambiguous`],
//!   and the jump waits for [`CONFIRM_FRAMES`] agreeing frames, as from a locked one: the frame
//!   alone cannot tell the two octaves apart, and the history says which one was playing. It
//!   shows the letter and the cents, which an octave error keeps, without the octave number
//!   (`docs/PRODUCT_SPEC.md` §2). Without such a peak, a jump from an estimate not yet locked is
//!   taken at once (an onset settling).
//! - **After a note.** While a locked note may still sound, two strings sound together, and
//!   read as neither: the guard says which estimates must prove they are one note
//!   ([`OctaveGuard::must_repeat`]), when the octave above a held
//!   note has entered ([`RISE`]), and when a reading is the lower string the estimate just left
//!   by moving up an octave ([`OctaveGuard::is_ringing_below`]).
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

/// A semitone, in cents: a reading within it of the locked note is that note dragged off its pitch
/// by another, not a new note ([`OctaveGuard::must_repeat`]).
pub const SEMITONE_CENTS: f64 = 100.0;

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

/// A period repeats: the NSDF of a signal periodic at `τ` peaks at `2τ` as high as at `τ`. After a
/// locked note, an estimate near it ([`OctaveGuard::must_repeat`]) is taken only if, where its range
/// can search `2τ`, its peak there (`Mpm::peak_near`) has at least this fraction of its clarity. A
/// chord's reading is the period of neither note, and does not repeat. Measured in `T-003c` on
/// every frame of a filled window, searching up to four lags from `2τ`: single notes, plucks with
/// inharmonicity to 10⁻³ and notes under a string ringing 12 dB down read at least 0.9895; the
/// third notes of two chords (A2 over a bass E, over a low B) at most 0.959. The threshold sits
/// between. Only the interfered notes needed more than one lag, and they are never tested: they
/// are clear and no octave of the note before (removing the wider search changed no figure).
pub const REPEATS: f32 = 0.975;

/// How far, as a fraction of the held note's clarity, its peak at τ/2 must rise over the lowest it
/// read since the note locked for the octave above to count as entering ([`OctaveGuard::judge`]).
pub const RISE: f32 = 0.5;

/// The octave above a note is *present* in a frame reading the note when the NSDF's level at half
/// its period has at least this fraction of its clarity. A note's own level there is
/// `(E_even − E_odd) / E`: a sawtooth −0.5, a weak fundamental (`synth::WEAK_FUNDAMENTAL`) 0.6, the
/// highest of the fixture spectra. A string an octave above sounding with it raises the level
/// toward 1 (G3 under G4 at 12 dB: 0.9). The threshold sits between.
pub const OCTAVE_ABOVE_PRESENT: f32 = 0.75;

/// While a locked note may still sound, an estimate whose range cannot test its octave is taken
/// only from this clarity, above [`UNVERIFIED_CLARITY`]. Such an estimate is a partial of a
/// fundamental the range cannot see, and a second note sounding skews it further: a bass E's
/// second partial read by Mid under an A2 ringing 12 dB down came out 52 ¢ off its pitch class at
/// clarity 0.753 (review round 4). Measured on filled windows, those blind readings sit at
/// ≤ 0.80 and correct blind readings at ≥ 0.9 (`T-003c` round 3; the threshold was checked on
/// held-out notes and detunings there). Only blind estimates pay it, only for the window the
/// locked note is remembered, and a withheld frame is no miss.
pub const AFTER_NOTE_CLARITY: f32 = 0.85;

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
///     guard.judge(at(110.0), None, None, 0.0, 16); // locked after a window's worth of frames
/// }
/// // A fifth up waits for three frames that agree; meanwhile there is nothing to show.
/// assert_eq!(guard.judge(at(164.8), None, None, 0.0, 16), None);
/// assert_eq!(guard.judge(at(164.8), None, None, 0.0, 16), None);
/// assert!(guard.judge(at(164.8), None, None, 0.0, 16).is_some());
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
    /// The last note shown with its octave known for a window's worth of frames in a row
    /// ([`OctaveGuard::shown`]). Kept through misses and range switches, forgotten only on
    /// silence: while it may still be sounding, some estimates must be periods
    /// ([`OctaveGuard::must_repeat`]).
    locked: Option<f64>,
    /// The note of the current run of frames shown known, and the run's length.
    known_run: Option<(f64, usize)>,
    /// Consecutive frames, since the last one that showed it, without the locked note: a window
    /// of them and it is no longer taken to be sounding ([`OctaveGuard::shown`]).
    without_locked: usize,
    /// The floor of the held note's τ/2 level ([`RISE`]).
    up_floor: Option<f32>,
    /// Since the octave above a held note entered: the level it rose from, and frames in a row
    /// without it ([`RISE`]).
    above: Option<(f32, usize)>,
    /// The note the estimate left by moving up an octave, and for how many frames in a row it has
    /// been read with the octave above absent from the frame ([`OctaveGuard::is_ringing_below`]).
    below: Option<(f64, usize)>,
}

impl OctaveGuard {
    /// A guard with nothing held.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything: silence.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// A range switch: the next frames are a different detector's, so nothing is held or locked
    /// for the hysteresis any more; the last locked note is remembered.
    pub fn switched(&mut self) {
        *self = Self {
            locked: self.locked,
            known_run: self.known_run,
            below: self.below,
            ..Self::default()
        };
    }

    /// The latest accepted estimate, in hertz.
    #[must_use]
    pub fn held(&self) -> Option<f64> {
        self.held
    }

    /// Whether `hz` is the note the estimate left by moving up an octave, read again while the
    /// octave above still shows in the frames that read it (the τ/2 level at least
    /// [`OCTAVE_ABOVE_PRESENT`] of the clarity, within the last window): the lower string still
    /// ringing under the one plucked an octave above it (G3 under G4, read by the range that sees
    /// both). Its octave is not the note's: show it ambiguous (`docs/adr/0029`).
    #[must_use]
    pub fn is_ringing_below(&self, hz: f64) -> bool {
        self.below
            .is_some_and(|(low, _)| cents(hz, low).abs() <= SAME_NOTE_CENTS)
    }

    /// Whether `hz` is the locked note itself, within [`SAME_NOTE_CENTS`]: a held note, however
    /// noisy, pays nothing for being the note before.
    #[must_use]
    pub fn is_locked_note(&self, hz: f64) -> bool {
        self.locked
            .is_some_and(|locked| cents(hz, locked).abs() <= SAME_NOTE_CENTS)
    }

    /// Whether a note has been locked since the last silence, and so may still be sounding under
    /// whatever comes next. `false` for an onset from silence.
    #[must_use]
    pub fn after_a_note(&self) -> bool {
        self.locked.is_some()
    }

    /// Whether `pick` must repeat at twice its period ([`REPEATS`]) to be taken: after a locked
    /// note, if it is barely clear (under [`UNVERIFIED_CLARITY`]) and dragged off that note (more
    /// than [`SAME_NOTE_CENTS`] from it, within a semitone), or an octave of it (its pitch class within [`SAME_NOTE_CENTS`], another octave). A new
    /// note further away is not tested, so a note after a short gap costs nothing (review round 5). Two notes sounding together read
    /// as neither, but near one of them: the last note dragged flat at the clarity floor, or its
    /// octave below, the mixture's quasi-period (A2 over a low B reads A1). A real new note is a
    /// period, and passes; a held melody over a bass, which does not repeat cleanly (0.72–0.82),
    /// is neither, and is not tested (`T-003c`).
    #[must_use]
    pub fn must_repeat(&self, pick: &PitchEstimate) -> bool {
        self.locked.is_some_and(|locked| {
            let off = cents(pick.frequency_hz, locked);
            let class = off - 1_200.0 * (off / 1_200.0).round();
            (pick.clarity < UNVERIFIED_CLARITY
                && off.abs() > SAME_NOTE_CENTS
                && off.abs() <= SEMITONE_CENTS)
                || (class.abs() <= SAME_NOTE_CENTS && off.abs() > JUMP_CENTS)
        })
    }

    /// A reading of `hz` by any range, before any test, shown or not: if it is the locked note,
    /// that note is still sounding, and [`Self::shown`] does not forget it yet.
    pub fn heard(&mut self, hz: f64) {
        if self.is_locked_note(hz) {
            self.without_locked = 0;
        }
    }

    /// What the frame finally showed: `hz`, and whether its octave was known. A run of
    /// `lock_frames` known frames of one note locks it; `forget_frames` frames in a row that
    /// neither show nor hear it ([`Self::heard`]) forget it.
    pub fn shown(
        &mut self,
        hz: Option<f64>,
        known: bool,
        lock_frames: usize,
        forget_frames: usize,
    ) {
        // The locked note is taken to be sounding only while some range still reads it. After the
        // longest window of any range (Low's, 100 ms: the chain passes it as `forget_frames`)
        // without a frame showing or hearing it (`Self::heard`), no window holds it any more, and
        // it is forgotten. A gap, a new string after a muted one, a melody over the old note's
        // silence: none of them pays for the note before any longer than that (`docs/adr/0029`,
        // review round 5). One window of the current range would be too short: an A2 ringing
        // 12 dB under a bass E is read by no range at all from 5 ms on, yet the frames that Mid
        // reads at 72 ms are its blend with the bass E, 40 ¢ off.
        let showing = self
            .locked
            .is_some_and(|locked| hz.is_some_and(|hz| cents(hz, locked).abs() <= SAME_NOTE_CENTS));
        self.without_locked = if showing { 0 } else { self.without_locked + 1 };
        if self.without_locked >= forget_frames {
            self.locked = None;
        }
        let Some(hz) = hz.filter(|_| known) else {
            self.known_run = None;
            return;
        };
        let run = match self.known_run {
            Some((note, run)) if cents(hz, note).abs() <= SAME_NOTE_CENTS => run + 1,
            _ => 1,
        };
        self.known_run = Some((hz, run));
        if run >= lock_frames {
            self.locked = Some(hz);
            self.without_locked = 0;
        }
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
        half: f32,
        lock_frames: usize,
    ) -> Option<Guarded> {
        self.misses = 0;
        let hz = pick.frequency_hz;
        let Some(held) = self.held.filter(|&held| cents(hz, held).abs() > JUMP_CENTS) else {
            let rose = self.octave_above_rose(&pick, half, lock_frames);
            let mut guarded = self.accept(pick, half, lock_frames);
            if rose {
                self.doubt = lock_frames;
                guarded.ambiguous = true;
            }
            return Some(guarded);
        };
        let agreeing = match self.pending {
            Some((previous, count)) if cents(hz, previous).abs() <= SAME_NOTE_CENTS => count + 1,
            _ => 1,
        };
        if agreeing >= CONFIRM_FRAMES {
            return Some(self.accept(pick, half, lock_frames));
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
        // (b): from an estimate not yet locked, a jump without a bridge is taken at once (an onset
        // settling); one with a bridge waited above, as from a locked one.
        if self.run < lock_frames {
            return Some(self.accept(pick, half, lock_frames));
        }
        self.pending = Some((hz, agreeing));
        None
    }

    /// Whether the octave above a locked, held note has just entered: its peak at τ/2 has risen by
    /// [`RISE`] over a floor that follows its lowest reading since the note locked, relaxing upward
    /// by a quarter of `RISE` per window. The level is the NSDF's value at
    /// τ/2, not a peak there: a peak barely formed comes and goes, and a weak fundamental's steady
    /// 0.6 would then read as a rise. A string plucked an octave above one
    /// still ringing, in a range that sees both, reads as the lower one; only this rise tells.
    fn octave_above_rose(&mut self, pick: &PitchEstimate, half: f32, lock_frames: usize) -> bool {
        let held = self
            .held
            .is_some_and(|held| cents(pick.frequency_hz, held).abs() <= SAME_NOTE_CENTS);
        if !held || self.run < lock_frames || pick.clarity <= 0.0 {
            return false;
        }
        let ratio = half / pick.clarity;
        // The floor relaxes upward by a quarter of RISE per window, so a level that drifts up as a
        // string decays (its fundamental faster than its second partial: 0.02 per Low window in
        // the fastest-drifting fixture) never adds up to a rise, while a string entering, which
        // raises the level by more than RISE within the window that fills with it, still does.
        let relax =
            RISE / (4.0 * crate::convert::to_f32(crate::convert::index_to_f64(lock_frames)));
        let floor = self
            .up_floor
            .map_or(ratio, |floor| (floor + relax).min(ratio));
        self.up_floor = Some(floor);
        // The rise is the onset; the level staying up is the octave above still sounding. Armed by
        // a rise, the doubt holds while the level stays at least half a RISE over the floor it rose
        // from, and lapses after a window without that.
        self.above = if ratio - floor >= RISE {
            Some((floor, 0))
        } else {
            self.above
                .map(|(base, absent)| {
                    let present = ratio >= base + 0.5 * RISE;
                    (base, if present { 0 } else { absent + 1 })
                })
                .filter(|&(_, absent)| absent < lock_frames)
        };
        self.above.is_some()
    }

    fn accept(&mut self, pick: PitchEstimate, half: f32, lock_frames: usize) -> Guarded {
        let hz = pick.frequency_hz;
        let previous = self.held;
        let same = previous.is_some_and(|held| cents(hz, held).abs() <= SAME_NOTE_CENTS);
        // The note the estimate left by moving up an octave: while the octave above still shows in
        // the frames that read it, it is the lower string still ringing; a window of frames
        // without the octave above, and it is a note again.
        let near = |a: f64, b: f64| cents(a, b).abs() <= SAME_NOTE_CENTS;
        let present = half >= OCTAVE_ABOVE_PRESENT * pick.clarity;
        self.below = match self.below {
            Some((low, absent)) if near(hz, low) => {
                let absent = if present { 0 } else { absent + 1 };
                (absent < lock_frames).then_some((low, absent))
            }
            Some((low, _)) if near(hz, 2.0 * low) => Some((low, 0)),
            _ => previous
                .filter(|&old| !same && near(hz, 2.0 * old))
                .map(|old| (old, 0)),
        };
        self.run = if same { self.run.saturating_add(1) } else { 1 };
        if !same {
            self.up_floor = None;
            self.above = None;
        }
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
            guard.judge(at(82.41, 0.99), None, None, 0.0, 16);
        }
        guard
    }

    #[test]
    fn a_jump_from_a_locked_estimate_holds_for_two_frames_and_gives_way_on_the_third() {
        // Holds: two frames of a fifth up, then the held note again, show nothing wrong.
        let mut guard = locked();
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        let back = guard.judge(at(82.4, 0.99), None, None, 0.0, 16).unwrap();
        assert!(!back.ambiguous && (back.estimate.frequency_hz - 82.4).abs() < 1e-9);
        // Gives way: three in a row.
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        assert_eq!(guard.judge(at(123.6, 0.95), None, None, 0.0, 16), None);
        let jumped = guard.judge(at(123.5, 0.95), None, None, 0.0, 16).unwrap();
        assert!((jumped.estimate.frequency_hz - 123.5).abs() < 1e-12);
        assert!(guard.held().is_some_and(|hz| (hz - 123.5).abs() < 1e-12));
        // A missing frame breaks the run; disagreeing frames restart it.
        let mut guard = locked();
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        guard.miss(16);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        assert_eq!(guard.judge(at(196.0, 0.95), None, None, 0.0, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        assert_eq!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16), None);
        assert!(guard.judge(at(123.5, 0.95), None, None, 0.0, 16).is_some());
    }

    #[test]
    fn an_estimate_not_yet_locked_and_a_glide_move_at_once() {
        let mut guard = OctaveGuard::new();
        for _ in 0..15 {
            guard.judge(at(82.41, 0.99), None, None, 0.0, 16);
        }
        assert!(guard.judge(at(164.8, 0.99), None, None, 0.0, 16).is_some());
        // A bend, 590 ¢ in 59 steps of 10 ¢, never jumps.
        let mut guard = locked();
        for step in 1..=59 {
            let hz = 82.41 * 2_f64.powf(f64::from(step) * 10.0 / 1_200.0);
            assert!(
                guard.judge(at(hz, 0.99), None, None, 0.0, 16).is_some(),
                "{step}"
            );
        }
        // Nothing held after a window of misses: the next note is taken at once.
        let mut guard = locked();
        for _ in 0..16 {
            guard.miss(16);
        }
        assert!(guard.judge(at(196.0, 0.99), None, None, 0.0, 16).is_some());
    }

    #[test]
    fn a_comparable_peak_at_the_held_octave_is_shown_ambiguous_at_the_comparable_edge() {
        let pick = at(164.82, 0.98);
        // 2τ, the octave below, is the held note.
        let edge = at(82.41, COMPARABLE * 0.98);
        let mut guard = locked();
        let shown = guard.judge(pick, None, Some(edge), 0.0, 16).unwrap();
        assert!(shown.ambiguous && shown.estimate == edge);
        // Just below the threshold it is not comparable: nothing is shown.
        let below = at(82.41, COMPARABLE * 0.98 - 1e-4);
        let mut guard = locked();
        assert_eq!(guard.judge(pick, None, Some(below), 0.0, 16), None);
        // τ/2, the octave above, likewise, when the held note is above.
        let mut guard = OctaveGuard::new();
        for _ in 0..16 {
            guard.judge(at(329.63, 0.99), None, None, 0.0, 16);
        }
        let up = at(329.6, 0.97);
        let shown = guard
            .judge(at(164.8, 0.99), Some(up), None, 0.0, 16)
            .unwrap();
        assert!(shown.ambiguous && shown.estimate == up);
        // The threshold in absolute terms, from `pitch::tests`' measured ratios: a weak
        // fundamental's second partial (0.6 of the note) is not comparable; 0.95 of it is.
        let mut guard = locked();
        assert_eq!(
            guard.judge(pick, None, Some(at(82.41, 0.6 * 0.98)), 0.0, 16),
            None
        );
        let mut guard = locked();
        assert!(
            guard
                .judge(pick, None, Some(at(82.41, 0.95 * 0.98)), 0.0, 16)
                .is_some()
        );
        // A comparable peak that is not the held note changes nothing.
        let mut guard = locked();
        assert_eq!(
            guard.judge(pick, Some(at(329.6, 0.99)), None, 0.0, 16),
            None
        );
    }

    #[test]
    fn a_pick_flipping_between_octaves_keeps_the_octave_ambiguous_for_a_window() {
        // A lower string ringing under the octave above: the pick flips every frame.
        let mut guard = locked();
        let (e2, e3) = (at(82.41, 1.0), at(164.82, 1.0));
        for _ in 0..40 {
            assert!(guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
            assert!(guard.judge(e2, Some(e3), None, 0.0, 16).unwrap().ambiguous);
        }
        // Once the upper one is gone, a window of agreeing frames makes it known again.
        for frame in 1..16 {
            assert!(
                guard.judge(e2, None, None, 0.0, 16).unwrap().ambiguous,
                "{frame}"
            );
        }
        assert!(!guard.judge(e2, None, None, 0.0, 16).unwrap().ambiguous);
    }

    #[test]
    fn after_a_locked_note_only_estimates_near_it_must_repeat() {
        let mut guard = OctaveGuard::new();
        // From silence nothing must repeat; nor after a run too short to lock, or not known.
        assert!(!guard.must_repeat(&at(55.0, 0.61)));
        for _ in 0..15 {
            guard.shown(Some(110.0), true, 16, 16);
        }
        assert!(!guard.after_a_note());
        guard.shown(Some(110.0), false, 16, 16);
        guard.shown(Some(110.0), true, 16, 16);
        assert!(!guard.after_a_note());
        for _ in 0..16 {
            guard.shown(Some(110.0), true, 16, 16);
        }
        assert!(guard.after_a_note());
        // Locked on A2: its octaves, and a barely clear reading within a semitone of it, must
        // repeat; another note, however barely clear, or the A2 itself, need not.
        assert!(guard.must_repeat(&at(55.0, 0.9)) && guard.must_repeat(&at(220.4, 0.9)));
        assert!(guard.must_repeat(&at(106.6, 0.6)) && !guard.must_repeat(&at(587.3, 0.74)));
        assert!(!guard.must_repeat(&at(587.3, 0.78)) && !guard.must_repeat(&at(108.0, 0.9)));
        assert!(!guard.must_repeat(&at(82.41, UNVERIFIED_CLARITY)));
        // A barely clear reading further than a semitone from the locked note is a new note: free.
        assert!(!guard.must_repeat(&at(98.0, 0.6)) && guard.must_repeat(&at(104.0, 0.6)));
        // A switch keeps the lock; silence forgets it.
        guard.switched();
        assert!(guard.must_repeat(&at(55.0, 0.9)));
        guard.reset();
        assert!(!guard.must_repeat(&at(55.0, 0.61)));
        // And so does a window of frames that do not show the locked note: a gap, another note.
        for _ in 0..16 {
            guard.shown(Some(110.0), true, 16, 16);
        }
        for frame in 0..16 {
            assert!(guard.after_a_note(), "{frame}");
            guard.shown(
                if frame % 2 == 0 { None } else { Some(82.41) },
                true,
                16,
                16,
            );
        }
        assert!(!guard.after_a_note());
    }

    #[test]
    fn a_jump_from_an_unlocked_estimate_waits_only_if_the_held_note_bridges_it() {
        let mut guard = OctaveGuard::new();
        for _ in 0..4 {
            guard.judge(at(82.41, 0.99), None, None, 0.0, 16);
        }
        // Not locked: a comparable peak at the held note bridges it, and the jump waits.
        let (e3, e2) = (at(164.82, 0.99), at(82.41, 0.99));
        assert!(guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
        assert!(guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
        assert!(!guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
        // Without one, it is taken at once.
        let mut guard = OctaveGuard::new();
        for _ in 0..4 {
            guard.judge(at(82.41, 0.99), None, None, 0.0, 16);
        }
        let taken = guard.judge(e3, None, None, 0.0, 16).unwrap();
        assert!(!taken.ambiguous && taken.estimate == e3);
    }

    #[test]
    fn the_octave_above_entering_a_held_note_makes_its_octave_ambiguous() {
        let mut guard = locked();
        let e2 = at(82.41, 1.0);
        // A window after the lock the floor is the held note's τ/2 level: a sawtooth's, −0.5.
        for _ in 0..16 {
            assert!(!guard.judge(e2, None, None, -0.5, 16).unwrap().ambiguous);
        }
        // Steady at a weak fundamental's 0.6 from the start would not rise; a step of RISE does.
        assert!(
            !guard
                .judge(e2, None, None, -0.5 + RISE - 0.01, 16)
                .unwrap()
                .ambiguous
        );
        assert!(guard.judge(e2, None, None, 0.86, 16).unwrap().ambiguous);
        // A note whose τ/2 level is steady never trips it.
        let mut weak = locked();
        for _ in 0..200 {
            assert!(!weak.judge(e2, None, None, 0.6, 16).unwrap().ambiguous);
        }
    }

    #[test]
    fn the_lower_string_left_by_an_octave_up_is_ringing_until_it_holds_alone_for_a_window() {
        let mut guard = OctaveGuard::new();
        for _ in 0..4 {
            guard.judge(at(196.0, 1.0), None, None, 0.0, 16);
        }
        // Up an octave, unlocked: taken at once; G3 is now the string below.
        guard.judge(at(392.0, 0.9), None, None, 0.0, 16);
        assert!(guard.is_ringing_below(196.0) && !guard.is_ringing_below(392.0));
        // Read again, alternating with the upper one, it stays the string below.
        for _ in 0..40 {
            guard.judge(at(196.0, 1.0), None, None, 0.9, 16);
            guard.judge(at(392.0, 0.9), None, None, 0.0, 16);
        }
        assert!(guard.is_ringing_below(196.0));
        // A window of frames of it alone, and it is a note again.
        for _ in 0..16 {
            guard.judge(at(196.0, 1.0), None, None, 0.1, 16);
        }
        assert!(!guard.is_ringing_below(196.0));
        // A new note is not; silence forgets it.
        guard.judge(at(392.0, 0.9), None, None, 0.0, 16);
        guard.judge(at(261.6, 0.9), None, None, 0.0, 16);
        assert!(!guard.is_ringing_below(196.0));
    }

    #[test]
    fn the_bridge_lasts_only_while_the_jump_waits() {
        // A leap from E2 to E3: E3's frame is periodic at 2τ too, so 2τ is always comparable. The
        // held E2 is shown, ambiguous, for two frames; the third takes E3.
        let mut guard = locked();
        let e3 = at(164.82, 0.999);
        let e2 = at(82.41, 0.999);
        for _ in 0..2 {
            assert!(guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
        }
        let third = guard.judge(e3, None, Some(e2), 0.0, 16).unwrap();
        assert!(!third.ambiguous && third.estimate == e3);
        assert!(!guard.judge(e3, None, Some(e2), 0.0, 16).unwrap().ambiguous);
    }
}
