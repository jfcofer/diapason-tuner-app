//! The tuner's analysis chain, split where the ring buffer splits it (`docs/AUDIO_ENGINE.md` §2).
//!
//! - [`Front`] runs in the audio callback: the DC block, then the gate. What crosses the ring
//!   buffer is the filtered block *and* the gate's state after it.
//! - [`Analysis`] runs on the analysis thread. It takes each block with its gate state and does:
//!   - the band-limit and the ×4 decimation;
//!   - the frames of every range, and `Mpm` on the selected range's;
//!   - the [`OctaveGuard`] on each estimate, and the [`RangeSelector`], with the frames of the
//!     ranges below the selected one that verify its octave (`docs/adr/0029`);
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
use crate::gate::{GateConfig, GateError, NoiseGate};
use crate::octave::{
    AFTER_NOTE_CLARITY, Guarded, Octave, OctaveGuard, REPEATS, SAME_NOTE_CENTS, UNVERIFIED_CLARITY,
};
use crate::pitch::{Mpm, MpmConfig, MpmError, PitchEstimate};
use crate::range::{
    DECIMATION, LOWEST_FUNDAMENTAL_HZ, Observation, Path, Range, RangeSelector, RangeTable,
};

/// Seconds the gate may stay open without a clear pitch before the analysis calls it noise and
/// the gate's floor learns from it ([`NoiseGate::set_unpitched`]). Long enough that a note's
/// attack, or a scan through the ranges (at most 150 ms per range), never counts; as long as the
/// floor's own memory, so the room is learned on the same time scale it is forgotten.
pub const UNPITCHED_SECONDS: f64 = 2.0;

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
/// use diapason_dsp::gate::GateConfig;
///
/// let mut front = Front::new(48_000, GateConfig::default())?;
/// let mut block = [0.1_f32; 480]; // a DC step at −20 dBFS: the gate opens, the DC goes
/// assert!(front.process(&mut block));
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
    pub fn process(&mut self, block: &mut [f32]) -> bool {
        self.dc.process(block);
        self.gate.process(block);
        self.gate.is_open()
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
    /// (`crate::octave`).
    pub estimate: Option<PitchEstimate>,
    /// Whether the estimate's octave is known, or only its pitch class and cents
    /// (`docs/PRODUCT_SPEC.md` §2, `ambiguous octave`). [`Octave::Known`] when there is no estimate.
    pub octave: Octave,
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
/// use diapason_dsp::range::Range;
/// use diapason_dsp::synth::sine;
///
/// let mut analysis = Analysis::new(48_000, Selection::Auto(Range::Mid))?;
/// let mut tone = vec![0.0_f32; 4_800];
/// sine(&mut tone, 110.0, 48_000, 0.5, 0.0);
/// let mut last = None;
/// analysis.push(&tone, true, |frame| last = frame.estimate);
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
    /// The latest clear reading of each range below the selected one (Low, Mid), in hertz, and
    /// the stream position of its frame: what corroborates a blind estimate
    /// ([`Self::corroborated`]).
    below: [Option<(f64, u64)>; 2],
    /// The clarity of the current range's latest estimate, before any test: what a watcher's
    /// reading must exceed ([`RangeSelector::observe_watcher`]).
    current_clarity: Option<f32>,
    position: u64,
    gate_open: bool,
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
            below: [None; 2],
            current_clarity: None,
            table,
            selection,
            position: 0,
            gate_open: false,
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
        self.gate_open && self.position.saturating_sub(self.pitched_at) >= self.unpitched_after
    }

    /// Take `block`, with the gate's state after it ([`Front::process`]), and hand each analysed
    /// frame of the selected range to `on_frame`.
    pub fn push(&mut self, block: &[f32], gate_open: bool, mut on_frame: impl FnMut(&Frame)) {
        self.gate_open = gate_open;
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
        if !gate_open {
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
            if !self.gate_open {
                self.guard.reset();
                self.observe(Observation::Silent);
                return;
            }
            let (estimate, octave) = self.judge(range);
            on_frame(&Frame {
                end: self.position,
                range,
                estimate,
                octave,
            });
        } else if matches!(self.selection, Selection::Auto(_)) && self.gate_open && range < current
        {
            // A verifier's reading must pass the same tests, or a chord's would verify an octave.
            let analysed = self.analyse(range);
            if let Some(e) = analysed {
                self.guard.heard(e.frequency_hz);
            }
            let raw = analysed.filter(|e| self.within_range(range, e));
            if let (Some(below), Some(e)) = (self.below.get_mut(slot(range)), raw) {
                *below = Some((e.frequency_hz, self.position));
            }
            let reading = raw
                .filter(|e| self.proven(range, e))
                .map(|e| e.frequency_hz);
            self.selector
                .observe_verifier(range, reading, self.position);
            if self.selector.current() != current {
                self.guard.switched();
            }
        } else if matches!(self.selection, Selection::Auto(_))
            && self.gate_open
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
        let raw = analysed.filter(|pick| self.within_range(range, pick));
        let pick = raw.filter(|pick| self.proven(range, pick));
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
        // An estimate under `UNVERIFIED_CLARITY` is shown (the locked note in noise, or a blind
        // one a lower range reads too, `Self::corroborated`), but once the selector has a pitch it
        // is no evidence of a change of note within the range's band, nor that a held note goes on:
        // `Observation::Faint`. (Out of the band it moves the chain as any pitch does: a quiet
        // melody note entering over a bass is first read faint, by the bass's range.)
        // Taken as a pitch, it restarts or delays the verifiers' freshness and the scan: High's
        // 0.6-clear E3 after an E4 delayed Mid by 25 ms; A2's last faint frames (0.73, 0.62 ...)
        // delayed Low by 10 ms.
        let faint = raw.is_some_and(|pick| {
            pick.clarity < UNVERIFIED_CLARITY && self.table.holds(range, pick.frequency_hz)
        }) && self.selector.pitch().is_some();
        // A pitch the range hears but may not show yet is not a miss (`Observation::Withheld`).
        let observation = match (hz, raw) {
            (Some(_), _) if faint => Observation::Faint,
            (Some(hz), _) => Observation::Pitch(hz),
            (None, Some(_)) => Observation::Withheld,
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
            (false, 0),
            "no frame is analysed with the gate closed"
        );
        let open = front.process(&mut tone);
        analysis.push(&tone, open, |_| frames += 1);
        // 9600 samples at 12 kHz / 4 is 2400 decimated samples: 80 hops of 30.
        assert_eq!((open, frames), (true, 80));
        assert_eq!(analysis.position(), 19_200);
    }

    #[test]
    fn unpitched_needs_the_gate_open_and_no_clear_pitch_for_two_seconds() {
        let mut analysis = Analysis::new(48_000, Selection::Auto(Range::Low)).unwrap();
        let mut noise = vec![0.0_f32; 48_000];
        crate::synth::Noise::new(4).fill(&mut noise, 0.05);
        analysis.push(&noise, false, |_| {});
        analysis.push(&noise, false, |_| {});
        assert!(!analysis.unpitched(), "a closed gate is not unpitched");
        analysis.push(&noise, true, |_| {});
        assert!(!analysis.unpitched());
        analysis.push(&noise, true, |_| {});
        assert!(analysis.unpitched());
        let mut tone = vec![0.0_f32; 9_600];
        sine(&mut tone, 220.0, 48_000, 0.5, 0.0);
        analysis.push(&tone, true, |_| {});
        assert!(!analysis.unpitched(), "a clear pitch resets it");
    }

    #[test]
    fn a_fixed_range_never_verifies_an_octave_it_cannot_test() {
        for (hz, expected) in [(82.41, Octave::Ambiguous), (110.0, Octave::Known)] {
            let mut analysis = Analysis::new(48_000, Selection::Fixed(Range::Mid)).unwrap();
            let mut tone = vec![0.0_f32; 48_000];
            crate::synth::sawtooth(&mut tone, hz, 48_000, 0.5, 0.0);
            let mut octaves = Vec::new();
            analysis.push(&tone, true, |frame| {
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
