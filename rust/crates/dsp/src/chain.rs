//! The tuner's analysis chain, split where the ring buffer splits it (`docs/AUDIO_ENGINE.md` §2).
//!
//! - [`Front`] runs in the audio callback: the DC block, then the gate. What crosses the ring
//!   buffer is the filtered block *and* the gate's state after it.
//! - [`Analysis`] runs on the analysis thread. It takes each block with its gate state and does:
//!   - the band-limit and the ×4 decimation;
//!   - the frames of every range, and `Mpm` on the selected range's;
//!   - the [`OctaveGuard`] on each estimate, and the [`RangeSelector`], with the frames of the
//!     ranges below the selected one that verify its octave (`docs/adr/0029`);
//!   - the [`Needle`] on the guarded estimate: what the needle shows (`docs/adr/0032`), held while
//!     two strings ring at once (`docs/adr/0033`);
//!   - the report, back to the gate, that the open gate has heard no pitch for a while
//!     ([`Analysis::unpitched`], [`Front::set_unpitched`]).
//!
//! So the engine (M3) owns only threads, the ring, and one flag going back (`AGENTS.md` §5).
//! Both types allocate only in their constructors; processing is allocation-free and panic-free
//! (`tests/no_alloc.rs`).

use std::fmt;

use crate::convert::{index_from_f64, index_to_f64, to_f32};
use crate::filter::{DcBlock, Decimator, FilterError};
use crate::frame::{FrameAssembler, FrameError};
use crate::gate::{GateConfig, GateError, GateState, NoiseGate};
use crate::octave::{
    AFTER_NOTE_CLARITY, CONFIRM_FRAMES, Guarded, Octave, OctaveGuard, REPEATS, SAME_NOTE_CENTS,
    UNVERIFIED_CLARITY,
};
use crate::pitch::{Mpm, MpmConfig, MpmError, PitchEstimate, partial_near};
use crate::range::{
    DECIMATION, LOWEST_FUNDAMENTAL_HZ, Observation, Path, Range, RangeSelector, RangeTable,
};
use crate::smooth::Needle;

/// Seconds the gate may stay open without a clear pitch before the analysis calls it noise and
/// the gate's floor learns from it ([`NoiseGate::set_unpitched`]). Long enough that a note's
/// attack, or a scan through the ranges (at most 150 ms per range), never counts; as long as the
/// floor's own memory, so the room is learned on the same time scale it is forgotten.
pub const UNPITCHED_SECONDS: f64 = 2.0;

/// The least share of a frame's power ([`crate::pitch::Partial::share`]) at a reading's own
/// frequency for a third or a quarter of the locked note to be a string, not two strings' common
/// period (`docs/adr/0033`). The common period of two strings has none: measured, at most
/// 10⁻⁵ of the frame. A string has its fundamental, weak or not: a guitar's low E with a weak one,
/// ringing under the E4 it is a quarter of, is 0.03.
pub const FUNDAMENTAL_SHARE: f64 = 1e-3;

/// Bins of a window (its rate over its length) a fundamental must lie above for
/// [`partial_near`] to tell it from its own octave: a Hann window's main lobe is four bins wide.
/// 40 Hz in Low's window. Below it a common period is never taken for a string: a weak low B's
/// peak, alone, lands 27.5 ¢ sharp, and G2's third, 32.67 Hz, over a low B, reads as one.
pub const RESOLVED_BINS: f64 = 4.0;

/// Why a chain could not be built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChainError {
    /// A filter, or the range table: an unsupported rate, in practice.
    Filter(FilterError),
    /// The gate's configuration.
    Gate(GateError),
    /// A frame shape.
    Frame(FrameError),
    /// A detector's configuration.
    Mpm(MpmError),
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filter(error) => write!(f, "filter: {error}"),
            Self::Gate(error) => write!(f, "gate: {error}"),
            Self::Frame(error) => write!(f, "frames: {error}"),
            Self::Mpm(error) => write!(f, "detector: {error}"),
        }
    }
}

impl std::error::Error for ChainError {}

impl From<FilterError> for ChainError {
    fn from(error: FilterError) -> Self {
        Self::Filter(error)
    }
}

impl From<GateError> for ChainError {
    fn from(error: GateError) -> Self {
        Self::Gate(error)
    }
}

impl From<FrameError> for ChainError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<MpmError> for ChainError {
    fn from(error: MpmError) -> Self {
        Self::Mpm(error)
    }
}

/// The real-time side: DC block, then gate.
///
/// ```
/// use diapason_dsp::chain::Front;
/// use diapason_dsp::gate::{GateConfig, GateState};
///
/// let mut front = Front::new(48_000, GateConfig::default())?;
/// let mut block = [0.1_f32; 480]; // a DC step at −20 dBFS: the gate opens, the DC goes
/// assert_eq!(front.process(&mut block), GateState::Open);
/// assert!(block[479].abs() < 0.1);
/// # Ok::<(), diapason_dsp::chain::ChainError>(())
/// ```
#[derive(Debug, Clone)]
pub struct Front {
    dc: DcBlock,
    gate: NoiseGate,
}

impl Front {
    /// The front end for `sample_rate`.
    ///
    /// # Errors
    ///
    /// [`ChainError`] for a rate the DC block or the gate rejects, or a bad gate configuration.
    pub fn new(sample_rate: u32, gate: GateConfig) -> Result<Self, ChainError> {
        Ok(Self {
            dc: DcBlock::new(sample_rate)?,
            gate: NoiseGate::new(sample_rate, gate)?,
        })
    }

    /// Filter `block` in place and run the gate over it; return the gate's state after the block,
    /// which travels across the ring buffer with it.
    pub fn process(&mut self, block: &mut [f32]) -> GateState {
        self.dc.process(block);
        self.gate.process(block);
        self.gate.state()
    }

    /// Pass on [`Analysis::unpitched`] (see [`NoiseGate::set_unpitched`]).
    pub fn set_unpitched(&mut self, unpitched: bool) {
        self.gate.set_unpitched(unpitched);
    }

    /// The gate, for its levels and thresholds.
    #[must_use]
    pub fn gate(&self) -> &NoiseGate {
        &self.gate
    }
}

/// How [`Analysis`] picks its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Always this range, and only its path runs: a range locked by hand, and how the accuracy
    /// and latency tests measure one range. No range below it is analysed, so nothing verifies an
    /// octave: an estimate the range cannot test itself stays [`Octave::Ambiguous`] for as long as
    /// it lasts (a low E, 82.41 Hz, in `Fixed(Range::Mid)`). That is the decision, not a gap: a
    /// range locked by hand has given up the ranges that could tell.
    Fixed(Range),
    /// The [`RangeSelector`], starting at, and returning to on silence, this home range.
    Auto(Range),
}

/// One analysed frame of the selected range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Input samples taken when the frame completed: its time on the audio clock.
    pub end: u64,
    /// The range it was analysed in.
    pub range: Range,
    /// What `Mpm` found, through the octave guard: `None` also while a jump waits for confirmation
    /// (`crate::octave`). Its frequency is `frequency_raw` (`docs/AUDIO_ENGINE.md` §3).
    pub estimate: Option<PitchEstimate>,
    /// `frequency_display`, in hertz: the estimate through the [`Needle`]'s median and one-euro
    /// filter. `None` with no estimate, and while a new note's first frames come in.
    pub display: Option<f64>,
    /// Whether the estimate's octave is known, or only its pitch class and cents
    /// (`docs/PRODUCT_SPEC.md` §2, `ambiguous octave`). [`Octave::Known`] when there is no estimate.
    pub octave: Octave,
    /// Two strings ring at once: a range read their common period within its last window, and the
    /// chain withheld it (`docs/adr/0033`). The estimate is `None`, so nothing locks on it, and the
    /// display holds the last value the needle showed of the note that rang before, flagged
    /// [`Octave::Ambiguous`].
    pub held: bool,
}

/// The analysis side. Under [`Selection::Auto`] both filters and all three frame assemblers run on
/// every sample, so a range switch finds its window already full and costs one hop, not a window
/// (`docs/adr/0028`). Frames are analysed only for the selected range, and only while the gate is
/// open. Each frame of a range below the selected one is analysed too, as a verifier: Mid and Low
/// under High, Low under Mid. They decide the octave of estimates the selected range cannot test
/// and catch a range locked on the octave above ([`RangeSelector::observe_verifier`]). Each
/// estimate passes the [`OctaveGuard`] first, with its NSDF's peaks at τ/2 and 2τ
/// ([`Mpm::peak_near`]).
///
/// ```
/// use diapason_dsp::chain::{Analysis, Selection};
/// use diapason_dsp::gate::GateState;
/// use diapason_dsp::range::Range;
/// use diapason_dsp::synth::sine;
///
/// let mut analysis = Analysis::new(48_000, Selection::Auto(Range::Mid))?;
/// let mut tone = vec![0.0_f32; 4_800];
/// sine(&mut tone, 110.0, 48_000, 0.5, 0.0);
/// let mut last = None;
/// analysis.push(&tone, GateState::Open, |frame| last = frame.estimate);
/// assert!((last.expect("a clear A2").frequency_hz - 110.0).abs() < 0.01);
/// # Ok::<(), diapason_dsp::chain::ChainError>(())
/// ```
pub struct Analysis {
    table: RangeTable,
    selection: Selection,
    selector: RangeSelector,
    decimator: Decimator,
    band_limit: Decimator,
    frames: [FrameAssembler; 3],
    detectors: [Mpm; 3],
    guard: OctaveGuard,
    display: Needle,
    /// The latest clear reading of each range below the selected one (Low, Mid), in hertz, and
    /// the stream position of its frame: what corroborates a blind estimate
    /// ([`Self::corroborated`]).
    below: [Option<(f64, u64)>; 2],
    /// The clarity of the current range's latest estimate, before any test: what a watcher's
    /// reading must exceed ([`RangeSelector::observe_watcher`]).
    current_clarity: Option<f32>,
    /// The position up to which a chord's common period is within the window of the range that read
    /// it ([`Self::in_chord`]).
    chord_until: u64,
    /// The locked note a chord was read over ([`OctaveGuard::anchor`]): until another note locks,
    /// the two strings may still both ring ([`Self::in_chord`]), even once the guard forgets the
    /// old note, whose string may still beat under the new one.
    chord_over: Option<f64>,
    /// Since when suspected common periods have had no fundamental, and from when that is longer
    /// than one takes to resolve: then the two strings are taken to ring, and frames are held
    /// ([`Frame::held`]). Before, a low string just plucked may only not be resolved yet.
    chord_since: Option<(u64, u64)>,
    /// Suspected common periods in a row that had a fundamental: [`CONFIRM_FRAMES`] of them and
    /// a string is sounding there, so the strings are no longer taken to ring together.
    strings: usize,
    /// What the needle last showed of the locked note ([`OctaveGuard::is_anchor_note`]): the last
    /// confident estimate, which a [`Frame::held`] frame shows.
    confident: Option<f64>,
    position: u64,
    gate: GateState,
    /// The position of the latest clear estimate, or of the latest closed-gate block.
    pitched_at: u64,
    unpitched_after: u64,
}

impl fmt::Debug for Analysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Analysis")
            .field("table", &self.table)
            .field("selection", &self.selection)
            .field("current", &self.current())
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

/// The index of a range's assembler and detector.
fn slot(range: Range) -> usize {
    match range {
        Range::Low => 0,
        Range::Mid => 1,
        Range::High => 2,
    }
}

impl Analysis {
    /// The analysis side for `sample_rate`, one of [`crate::range::SUPPORTED_RATES`].
    ///
    /// # Errors
    ///
    /// [`ChainError::Filter`] for an unsupported rate.
    pub fn new(sample_rate: u32, selection: Selection) -> Result<Self, ChainError> {
        let table = RangeTable::new(sample_rate)?;
        let home = match selection {
            Selection::Fixed(range) | Selection::Auto(range) => range,
        };
        let [low, mid, high] = Range::ALL.map(|range| table.spec(range));
        let frames = [
            FrameAssembler::new(low.window, low.hop)?,
            FrameAssembler::new(mid.window, mid.hop)?,
            FrameAssembler::new(high.window, high.hop)?,
        ];
        let detectors = [
            Mpm::new(MpmConfig::with_window(low.window))?,
            Mpm::new(MpmConfig::with_window(mid.window))?,
            Mpm::new(MpmConfig::with_window(high.window))?,
        ];
        let unpitched_after = index_from_f64((UNPITCHED_SECONDS * f64::from(sample_rate)).round());
        Ok(Self {
            selector: RangeSelector::new(&table, home),
            decimator: table.decimator()?,
            band_limit: table.band_limit()?,
            frames,
            detectors,
            guard: OctaveGuard::new(),
            display: Needle::new(sample_rate),
            below: [None; 2],
            current_clarity: None,
            chord_until: 0,
            chord_over: None,
            chord_since: None,
            strings: 0,
            confident: None,
            table,
            selection,
            position: 0,
            gate: GateState::Closed,
            pitched_at: 0,
            unpitched_after: u64::try_from(unpitched_after).unwrap_or(u64::MAX),
        })
    }

    /// The range table in use.
    #[must_use]
    pub fn table(&self) -> &RangeTable {
        &self.table
    }

    /// The range the next frame is analysed in.
    #[must_use]
    pub fn current(&self) -> Range {
        match self.selection {
            Selection::Fixed(range) => range,
            Selection::Auto(_) => self.selector.current(),
        }
    }

    /// Input samples taken so far: the audio clock.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Whether the gate has been open for [`UNPITCHED_SECONDS`] with no clear estimate: the open
    /// gate is hearing noise. The engine hands this to [`Front::set_unpitched`].
    #[must_use]
    pub fn unpitched(&self) -> bool {
        self.gate.is_open() && self.position.saturating_sub(self.pitched_at) >= self.unpitched_after
    }

    /// Take `block`, with the gate's state after it ([`Front::process`]), and hand each analysed
    /// frame of the selected range to `on_frame`.
    pub fn push(&mut self, block: &[f32], gate: GateState, mut on_frame: impl FnMut(&Frame)) {
        self.gate = gate;
        for &sample in block {
            self.position += 1;
            if self.runs(Path::Full)
                && let Some(full) = self.band_limit.process_sample(sample)
            {
                self.feed(Range::High, full, &mut on_frame);
            }
            if self.runs(Path::Decimated)
                && let Some(decimated) = self.decimator.process_sample(sample)
            {
                self.feed(Range::Low, decimated, &mut on_frame);
                self.feed(Range::Mid, decimated, &mut on_frame);
            }
        }
        if !gate.is_open() {
            self.pitched_at = self.position;
        }
    }

    fn runs(&self, path: Path) -> bool {
        match self.selection {
            Selection::Fixed(range) => range.path() == path,
            Selection::Auto(_) => true,
        }
    }

    fn analyse(&mut self, range: Range) -> Option<PitchEstimate> {
        let rate = self.table.spec(range).analysis_rate;
        let index = slot(range);
        let window = self.frames.get(index)?.window();
        let estimate = self.detectors.get_mut(index)?.analyze(window, rate);
        if estimate.is_some() {
            self.pitched_at = self.position;
        }
        estimate
    }

    fn feed(&mut self, range: Range, sample: f32, on_frame: &mut impl FnMut(&Frame)) {
        let completed = self
            .frames
            .get_mut(slot(range))
            .is_some_and(|frames| frames.push_sample(sample));
        if !completed {
            return;
        }
        let current = self.current();
        if range == current {
            if !self.gate.is_open() {
                self.guard.reset();
                self.display.reset();
                self.release_chord();
                self.observe(Observation::Silent);
                return;
            }
            let (estimate, octave) = self.judge(range);
            // Another note has locked: whatever rang with the old one, this one is no chord.
            if self.chord_over.is_some_and(|over| {
                self.guard.anchor().is_some() && !self.guard.is_anchor_note(over)
            }) {
                self.release_chord();
            }
            let held = estimate.is_none()
                && self.in_chord()
                && self
                    .chord_since
                    .is_some_and(|(_, confirmed)| self.position >= confirmed);
            // A held frame leaves the needle's history alone: no miss, so the note it held goes on
            // when the chord resolves back to it.
            let display = if held {
                self.confident
            } else {
                self.display
                    .update(estimate.map(|e| e.frequency_hz), self.position)
            };
            if let (Some(e), Some(shown)) = (estimate, display)
                && self.guard.is_anchor_note(e.frequency_hz)
            {
                self.confident = Some(shown);
            }
            // While two strings ring the octave is not decided: a held frame shows the letter and the
            // cents of the note held, not its octave (a low E under the E4 it is a quarter of).
            let octave = if held { Octave::Ambiguous } else { octave };
            on_frame(&Frame {
                end: self.position,
                range,
                estimate,
                display,
                octave,
                held,
            });
        } else if matches!(self.selection, Selection::Auto(_))
            && self.gate.is_open()
            && range < current
        {
            // A verifier's reading must pass the same tests, or a chord's would verify an octave.
            let analysed = self.analyse(range);
            if let Some(e) = analysed {
                self.guard.heard(e.frequency_hz);
            }
            let chord = self.chord(range, analysed);
            let raw = analysed.filter(|e| !chord && self.within_range(range, e));
            if let (Some(below), Some(e)) = (self.below.get_mut(slot(range)), raw) {
                *below = Some((e.frequency_hz, self.position));
            }
            let reading = raw
                .filter(|e| self.proven(range, e) && self.one_string(range, e))
                .map(|e| e.frequency_hz);
            self.selector
                .observe_verifier(range, reading, self.position);
            if self.selector.current() != current {
                self.guard.switched();
            }
        } else if matches!(self.selection, Selection::Auto(_))
            && self.gate.is_open()
            && current.above() == Some(range)
        {
            let reading = self
                .analyse(range)
                .filter(|e| self.within_range(range, e))
                .map(|e| (e.frequency_hz, e.clarity));
            self.selector
                .observe_watcher(range, reading, self.current_clarity, self.position);
            if self.selector.current() != current {
                self.guard.switched();
            }
        }
    }

    /// The selected range's frame: `Mpm`, the octave guard, the selector, then the octave.
    fn judge(&mut self, range: Range) -> (Option<PitchEstimate>, Octave) {
        let lock = self.selector.held_frames(range);
        let after_a_note = self.guard.after_a_note();
        // A period longer than the lowest note's is a common period of two notes, not a string; an
        // estimate the range cannot test the octave of is taken only from `UNVERIFIED_CLARITY`; and
        // after a note, which may still sound, an estimate near it must be a period, which a
        // chord's is not (`OctaveGuard::must_repeat`).
        let analysed = self.analyse(range);
        self.current_clarity = analysed.map(|e| e.clarity);
        if let Some(e) = analysed {
            self.guard.heard(e.frequency_hz);
        }
        let chord = self.chord(range, analysed);
        let raw = analysed.filter(|pick| self.within_range(range, pick));
        let pick =
            raw.filter(|pick| !chord && self.proven(range, pick) && self.one_string(range, pick));
        let guarded = if let Some(pick) = pick {
            let up = self.peak_near(range, pick.period_samples / 2.0);
            let down = self.peak_near(range, pick.period_samples * 2.0);
            let half = self.value_at(range, pick.period_samples / 2.0);
            self.guard.judge(pick, up, down, half, lock)
        } else {
            self.guard.miss(lock);
            None
        };
        let hz = guarded.map(|g| g.estimate.frequency_hz);
        // An estimate under `UNVERIFIED_CLARITY` in the range's band is shown (the locked note in
        // noise, or a blind one a lower range reads too, `Self::corroborated`), but once the
        // selector has a pitch it is withheld from it: no evidence of a change of note, nor that a
        // held note goes on. Taken as a pitch, it restarts or delays the verifiers' freshness:
        // High's 0.6-clear E3 after an E4 delayed Mid by 25 ms. Out of the band it moves the chain
        // as any pitch does: a quiet melody note entering over a bass is first read faint, by the
        // bass's range. (Before `Observation::Holding`, counted as a miss, a string's last faint
        // frames after a 0.15 s gap scanned away from the next note's range: A3 → A2 151 ms, not
        // 79. Since then no gated test needs this rule, as a miss or as a pitch: `T-003c`.)
        let faint = raw.is_some_and(|pick| {
            pick.clarity < UNVERIFIED_CLARITY && self.table.holds(range, pick.frequency_hz)
        }) && self.selector.pitch().is_some();
        // A pitch the range hears but may not show yet is not a miss (`Observation::Withheld`), nor
        // is a frame without one while the gate only holds (`Observation::Holding`).
        let observation = match (hz, raw) {
            (Some(_), _) if faint => Observation::Withheld,
            (Some(hz), _) => Observation::Pitch(hz),
            (None, Some(_)) => Observation::Withheld,
            (None, None) if self.gate == GateState::Holding => Observation::Holding,
            (None, None) => Observation::Unclear,
        };
        self.observe(observation);
        if self.current() != range {
            self.guard.switched();
        }
        let octave = match guarded {
            Some(Guarded {
                ambiguous: true, ..
            }) => Octave::Ambiguous,
            Some(Guarded { estimate, .. }) => {
                let hz = estimate.frequency_hz;
                let verified = matches!(self.selection, Selection::Auto(_))
                    && self.selector.verified(range, hz);
                // After a note, a period the range cannot test for repeating may be a chord's:
                // provisional until a verifier, which can, reads it. Low has none below it.
                let provisional =
                    after_a_note && range != Range::Low && !self.searches_double(range, &estimate);
                let below = self.guard.is_ringing_below(hz);
                if !below && (verified || (self.table.tests_octave(range, hz) && !provisional)) {
                    Octave::Known
                } else {
                    Octave::Ambiguous
                }
            }
            None => Octave::Known,
        };
        let shown = guarded.map(|g| g.estimate.frequency_hz);
        self.guard.shown(
            shown,
            octave == Octave::Known,
            lock,
            self.forget_frames(range),
        );
        (guarded.map(|g| g.estimate), octave)
    }

    /// Whether `analysed`, `range`'s reading, is the common period of two strings ringing at once
    /// (`docs/adr/0033`): 3/2, 3 or 4 times it is the locked note ([`OctaveGuard::is_common_period`]),
    /// which still sounds, and it has no fundamental of its own ([`Self::sounds`]). If so, the
    /// locked note is still sounding, and frames without an estimate are [`Frame::held`].
    fn chord(&mut self, range: Range, analysed: Option<PitchEstimate>) -> bool {
        let Some((hz, anchor)) = analysed
            .map(|e| e.frequency_hz)
            .filter(|&hz| self.guard.is_common_period(hz))
            .zip(self.guard.anchor())
        else {
            return false;
        };
        // A note left for one a fifth below it is no chord: the old string has stopped. Only the
        // latch goes: this test flickers under a near-miss chord (G2 over a low B), so the last
        // reading's window still asks for one string, and a held needle keeps its value.
        if !self.rings(range, anchor) {
            self.chord_over = None;
            self.chord_since = None;
            return false;
        }
        if self.sounds(range, hz) {
            // A string is sounding there. Confirmed, whatever rang before is no chord with it.
            self.strings += 1;
            if self.strings >= CONFIRM_FRAMES {
                self.release_chord();
            }
            return false;
        }
        self.strings = 0;
        self.guard.ringing();
        if self.position > self.chord_until {
            self.chord_since = None;
        }
        let resolve = self.resolving_samples(range, hz);
        self.chord_since
            .get_or_insert((self.position, self.position + resolve));
        self.chord_until = self.position + self.selector.span(range);
        self.chord_over = Some(anchor);
        true
    }

    /// Two strings no longer ring together: the old one stopped, a string sounds at the reading,
    /// another note locked, or the gate closed. Nothing is held, nor must repeat, any more.
    fn release_chord(&mut self) {
        self.chord_until = 0;
        self.chord_over = None;
        self.chord_since = None;
        self.strings = 0;
        self.confident = None;
    }

    /// Input samples it takes [`Self::sounds`] to resolve `hz` from its octave: the latest
    /// [`RESOLVED_BINS`] periods of the bin width, at the rate of the window it would use.
    fn resolving_samples(&self, range: Range, hz: f64) -> u64 {
        let spec = self.table.spec(range);
        let factor = match spec.path {
            Path::Decimated => DECIMATION,
            Path::Full => 1,
        };
        let needed = index_from_f64((RESOLVED_BINS * f64::from(spec.analysis_rate) / hz).ceil());
        let samples = needed
            .saturating_mul(factor)
            .min(self.table.spec(Range::Low).window * DECIMATION);
        u64::try_from(samples).unwrap_or(u64::MAX)
    }

    /// Whether two strings may be ringing together: a range read their common period within its
    /// window, or since then no other note has locked and the one they rang over is remembered.
    /// The second keeps the old string's beating off the new one: a D3 sine 10 dB under the G3
    /// it was a fourth below bent High's estimate of G3 by up to 65 ¢, after every range had
    /// stopped reading their common period (`docs/adr/0033`).
    fn in_chord(&self) -> bool {
        self.position <= self.chord_until
            || self.chord_over.is_some_and(|over| {
                self.guard.anchor().is_none() || self.guard.is_anchor_note(over)
            })
    }

    /// Whether `pick` may be shown while two strings ring ([`Frame::held`]): the note held, as it
    /// locked ([`OctaveGuard::is_anchor_note`]), or a period that repeats at twice itself ([`REPEATS`]) in the first range, from `range` down,
    /// that can search there ([`Self::repeats_in`]). Two strings read as a period of neither
    /// between their common period's multiples, and that does not repeat; the string just plucked
    /// does, once it rings clear. Two sines, A2 and D3, read 131.8 Hz in Mid; D3 and G3 read their
    /// mean, 174.5 Hz, in High, which cannot search 2τ there, and in Mid, which can. No range below
    /// Low: a reading no range can test is withheld while the chord rings.
    fn one_string(&self, range: Range, pick: &PitchEstimate) -> bool {
        let hz = pick.frequency_hz;
        let below = match self.selection {
            Selection::Auto(_) => [Range::Mid, Range::Low],
            Selection::Fixed(fixed) => [fixed, fixed],
        };
        !self.in_chord()
            || self.guard.is_anchor_note(hz)
            || [range]
                .into_iter()
                .chain(below.into_iter().filter(|&lower| lower < range))
                .find_map(|range| self.repeats_in(range, hz))
                .unwrap_or(false)
    }

    /// Whether `hz` repeats at twice its period in `range`'s last analysed frame: its NSDF's peak
    /// near `2τ` at least [`REPEATS`] of its peak near `τ`. `None` where `range` cannot search `2τ`.
    fn repeats_in(&self, range: Range, hz: f64) -> Option<bool> {
        let index = slot(range);
        let detector = self.detectors.get(index)?;
        let lag = f64::from(self.table.spec(range).analysis_rate) / hz;
        if 2.0 * lag + 1.0 >= index_to_f64(detector.max_lag()) {
            return None;
        }
        let at = self.peak_near(range, lag);
        let double = self.peak_near(range, 2.0 * lag);
        Some(
            at.zip(double)
                .is_some_and(|(at, double)| double.clarity >= REPEATS * at.clarity),
        )
    }

    /// Whether a partial sounds at `hz` itself ([`partial_near`]), in the latest samples of `range`'s
    /// window, or else of Low's, the longest, if the decimated path runs: as few as resolve `hz`
    /// from its octave ([`RESOLVED_BINS`]), so that as little as possible of them is from before
    /// the string was plucked. A whole window spanning the onset puts a low E's fundamental, under
    /// the E4 it is a quarter of, 50 to 150 ¢ flat for 70 ms. The first window long enough decides;
    /// a partial no window resolves is taken to be absent.
    fn sounds(&self, range: Range, hz: f64) -> bool {
        [range, Range::Low]
            .into_iter()
            .filter(|&range| self.runs(range.path()))
            .find_map(|range| {
                let rate = self.table.spec(range).analysis_rate;
                let window = self.frames.get(slot(range))?.window();
                let needed = index_from_f64((RESOLVED_BINS * f64::from(rate) / hz).ceil());
                let tail = window.get(window.len().checked_sub(needed)?..)?;
                Some(partial_near(tail, rate, hz))
            })
            .flatten()
            .is_some_and(|partial| present(partial, hz))
    }

    /// Whether a partial sounds at `hz` in `range`'s whole window ([`partial_near`]): the note
    /// that rang before still does. The whole window, because the note was there before the onset
    /// and the next string's partials must fall outside its main lobe: B3's under E4 are 83 Hz
    /// apart, inside the lobe of the 16 ms that would resolve B3 from its octave.
    fn rings(&self, range: Range, hz: f64) -> bool {
        let rate = self.table.spec(range).analysis_rate;
        self.frames
            .get(slot(range))
            .and_then(|frames| partial_near(frames.window(), rate, hz))
            .is_some_and(|partial| present(partial, hz))
    }

    /// Frames of `range` that span Low's window, the longest: how long a locked note no range
    /// reads may still sound in some range's window ([`OctaveGuard::shown`]).
    fn forget_frames(&self, range: Range) -> usize {
        let spec = self.table.spec(range);
        let hop = match spec.path {
            Path::Decimated => spec.hop * DECIMATION,
            Path::Full => spec.hop,
        };
        (self.table.spec(Range::Low).window * DECIMATION).div_ceil(hop.max(1))
    }

    /// Whether `range` can show `pick` at all: no lower than the lowest note, and clear enough if
    /// the range cannot test its octave. A frame failing this is a miss, like an unclear one: the
    /// range does not hear the note, and the selector should scan for one that does.
    fn within_range(&self, range: Range, pick: &PitchEstimate) -> bool {
        let hz = pick.frequency_hz;
        hz >= LOWEST_FUNDAMENTAL_HZ
            && (pick.clarity >= UNVERIFIED_CLARITY
                || self.table.tests_octave(range, hz)
                || (self.corroborated(range, hz)
                    && (!self.guard.after_a_note() || self.guard.is_locked_note(hz))))
    }

    /// Whether a range below `range`, analysed alongside, read the same note in a frame of the
    /// last window of its own (its span: the same sound, not a note before it).
    /// A blind estimate is either the note itself, which a lower range that sees its octave below
    /// reads too, or a partial of a lower fundamental, which it reads as that fundamental: so a
    /// noisy note is told from a partial frame by frame, where clarity cannot tell them apart
    /// (a G2 at −12 dB SNR reads 0.6–0.75, like a weak bass E's second partial).
    fn corroborated(&self, range: Range, hz: f64) -> bool {
        Range::ALL
            .into_iter()
            .filter(|&below| below < range)
            .filter_map(|below| {
                let (reading, end) = self.below.get(slot(below)).copied().flatten()?;
                let span = self.selector.span(below);
                (self.position.saturating_sub(end) <= span).then_some(reading)
            })
            .any(|reading| (1_200.0 * (reading / hz).log2()).abs() <= SAME_NOTE_CENTS)
    }

    /// Whether `pick` has proved it is one note while the last one may still sound. A frame
    /// failing only this is withheld, not missed: the range hears a pitch it may not show yet.
    fn proven(&self, range: Range, pick: &PitchEstimate) -> bool {
        let blind = !self.table.tests_octave(range, pick.frequency_hz);
        (!self.guard.must_repeat(pick) || self.repeats(range, pick))
            && !(blind
                && !self.corroborated(range, pick.frequency_hz)
                && self.guard.after_a_note()
                && !self.guard.is_locked_note(pick.frequency_hz)
                && pick.clarity < AFTER_NOTE_CLARITY)
    }

    /// Whether `range` can search twice `pick`'s period.
    fn searches_double(&self, range: Range, pick: &PitchEstimate) -> bool {
        self.detectors.get(slot(range)).is_some_and(|detector| {
            2.0 * pick.period_samples + 1.0 < index_to_f64(detector.max_lag())
        })
    }

    /// Whether `pick`'s period repeats at twice it ([`REPEATS`]), or its range cannot search there.
    fn repeats(&self, range: Range, pick: &PitchEstimate) -> bool {
        if !self.searches_double(range, pick) {
            return true;
        }
        let index = slot(range);
        let rate = self.table.spec(range).analysis_rate;
        let (Some(frames), Some(detector)) = (self.frames.get(index), self.detectors.get(index))
        else {
            return false;
        };
        let lag = 2.0 * pick.period_samples;
        detector
            .peak_near(frames.window(), rate, lag)
            .is_some_and(|double| double.clarity >= REPEATS * pick.clarity)
    }

    /// The selected range's NSDF at `lag`, from the frame just analysed; 0 where it has none.
    fn value_at(&self, range: Range, lag: f64) -> f32 {
        self.detectors
            .get(slot(range))
            .and_then(|detector| detector.value_at(lag))
            .map_or(0.0, to_f32)
    }

    /// The selected range's NSDF peak near `lag`, from the frame just analysed.
    fn peak_near(&self, range: Range, lag: f64) -> Option<PitchEstimate> {
        let index = slot(range);
        let rate = self.table.spec(range).analysis_rate;
        let window = self.frames.get(index)?.window();
        self.detectors.get(index)?.peak_near(window, rate, lag)
    }

    fn observe(&mut self, observation: Observation) {
        if let Selection::Auto(_) = self.selection {
            self.selector.observe(observation, self.position);
        }
    }
}

/// Whether `partial` is a partial at `hz`: within [`SAME_NOTE_CENTS`] of it, at least
/// [`FUNDAMENTAL_SHARE`] of the frame.
fn present(partial: crate::pitch::Partial, hz: f64) -> bool {
    partial.share >= FUNDAMENTAL_SHARE
        && (1_200.0 * (partial.frequency_hz / hz).log2()).abs() <= SAME_NOTE_CENTS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;

    #[test]
    fn the_gate_state_travels_with_the_block() {
        let mut front = Front::new(48_000, GateConfig::default()).unwrap();
        let mut analysis = Analysis::new(48_000, Selection::Fixed(Range::Mid)).unwrap();
        let mut tone = vec![0.0_f32; 9_600];
        sine(&mut tone, 110.0, 48_000, 0.5, 0.0);
        let mut silence = vec![0.0_f32; 9_600];
        let mut frames = 0;
        let open = front.process(&mut silence);
        analysis.push(&silence, open, |_| frames += 1);
        assert_eq!(
            (open, frames),
            (GateState::Closed, 0),
            "no frame is analysed with the gate closed"
        );
        let open = front.process(&mut tone);
        analysis.push(&tone, open, |_| frames += 1);
        // 9600 samples at 12 kHz / 4 is 2400 decimated samples: 80 hops of 30.
        assert_eq!((open, frames), (GateState::Open, 80));
        assert_eq!(analysis.position(), 19_200);
    }

    #[test]
    fn unpitched_needs_the_gate_open_and_no_clear_pitch_for_two_seconds() {
        let mut analysis = Analysis::new(48_000, Selection::Auto(Range::Low)).unwrap();
        let mut noise = vec![0.0_f32; 48_000];
        crate::synth::Noise::new(4).fill(&mut noise, 0.05);
        analysis.push(&noise, GateState::Closed, |_| {});
        analysis.push(&noise, GateState::Closed, |_| {});
        assert!(!analysis.unpitched(), "a closed gate is not unpitched");
        analysis.push(&noise, GateState::Open, |_| {});
        assert!(!analysis.unpitched());
        analysis.push(&noise, GateState::Open, |_| {});
        assert!(analysis.unpitched());
        let mut tone = vec![0.0_f32; 9_600];
        sine(&mut tone, 220.0, 48_000, 0.5, 0.0);
        analysis.push(&tone, GateState::Open, |_| {});
        assert!(!analysis.unpitched(), "a clear pitch resets it");
    }

    #[test]
    fn a_fixed_range_never_verifies_an_octave_it_cannot_test() {
        for (hz, expected) in [(82.41, Octave::Ambiguous), (110.0, Octave::Known)] {
            let mut analysis = Analysis::new(48_000, Selection::Fixed(Range::Mid)).unwrap();
            let mut tone = vec![0.0_f32; 48_000];
            crate::synth::sawtooth(&mut tone, hz, 48_000, 0.5, 0.0);
            let mut octaves = Vec::new();
            analysis.push(&tone, GateState::Open, |frame| {
                if frame.end > 4_800 && frame.estimate.is_some() {
                    octaves.extend([frame.octave]);
                }
            });
            assert!(octaves.len() > 300, "{hz} Hz");
            assert!(octaves.iter().all(|&octave| octave == expected), "{hz} Hz");
        }
    }

    #[test]
    fn an_unsupported_rate_is_an_error() {
        assert_eq!(
            Analysis::new(96_000, Selection::Auto(Range::Low)).unwrap_err(),
            ChainError::Filter(FilterError::SampleRate(96_000))
        );
        assert!(Front::new(0, GateConfig::default()).is_err());
    }
}
