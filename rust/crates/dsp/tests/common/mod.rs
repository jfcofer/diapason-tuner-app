//! The tuner's analysis chain, wired the way `docs/AUDIO_ENGINE.md` §2 draws it, for the
//! integration tests: DC block → gate → band-limit or ×4 decimation → frames per range → `Mpm`,
//! with a [`RangeSelector`] choosing the range.
//!
//! Wiring, not logic: every stage is a `diapason_dsp` type with its own tests. Which thread runs
//! which stage is the engine's decision (M3), so the composition lives here rather than in the
//! crate. It allocates only in [`Chain::new`]; `tests/no_alloc.rs` drives [`Chain::push`] with the
//! trap armed.
//!
//! Under the selector, both filters and all three frame assemblers run on every sample, whichever
//! range is selected, so a range switch finds its history already full and costs one hop, not a
//! window. Held in one range, only that range's path runs, since nothing else is ever analysed.
//! Frames are analysed only for the selected range, and only while the gate is open.

use diapason_dsp::filter::{DcBlock, Decimator};
use diapason_dsp::frame::FrameAssembler;
use diapason_dsp::gate::{GateConfig, NoiseGate};
use diapason_dsp::pitch::{Mpm, MpmConfig, PitchEstimate};
use diapason_dsp::range::{DECIMATION, Observation, Path, Range, RangeSelector, RangeTable};

/// How the chain picks its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// Always this range: accuracy and latency are properties of a range.
    Fixed(Range),
    /// The [`RangeSelector`], starting at and returning to this home range.
    Auto(Range),
}

/// One analysed frame.
#[derive(Debug, Clone, Copy)]
pub struct Event {
    /// Input samples taken when the frame completed: the audio clock.
    pub end: u64,
    /// The range the frame was analysed in.
    pub range: Range,
    /// What `Mpm` found.
    pub estimate: Option<PitchEstimate>,
}

pub struct Chain {
    table: RangeTable,
    selection: Selection,
    selector: RangeSelector,
    dc: DcBlock,
    gate: NoiseGate,
    decimator: Decimator,
    band_limit: Decimator,
    frames: [FrameAssembler; 3],
    detectors: [Mpm; 3],
    position: u64,
    /// Frames ending before this input position are not analysed (accuracy tests skip the
    /// warm-up, which is all wasted work in a debug build).
    analyse_from: u64,
}

fn slot(range: Range) -> usize {
    match range {
        Range::Low => 0,
        Range::Mid => 1,
        Range::High => 2,
    }
}

impl Chain {
    pub fn new(sample_rate: u32, selection: Selection) -> Self {
        let table = RangeTable::new(sample_rate).expect("a supported rate");
        let home = match selection {
            Selection::Fixed(range) | Selection::Auto(range) => range,
        };
        let shape = |range| table.spec(range);
        Self {
            selector: RangeSelector::new(&table, home),
            dc: DcBlock::new(sample_rate).expect("a supported rate"),
            gate: NoiseGate::new(sample_rate, GateConfig::default()).expect("default gate"),
            decimator: table.decimator().expect("designed"),
            band_limit: table.band_limit().expect("designed"),
            frames: Range::ALL.map(|range| {
                FrameAssembler::new(shape(range).window, shape(range).hop).expect("valid shape")
            }),
            detectors: Range::ALL.map(|range| {
                Mpm::new(MpmConfig::with_window(shape(range).window)).expect("valid window")
            }),
            table,
            selection,
            position: 0,
            analyse_from: 0,
        }
    }

    pub fn table(&self) -> &RangeTable {
        &self.table
    }

    pub fn analyse_from(&mut self, position: u64) {
        self.analyse_from = position;
    }

    pub fn current(&self) -> Range {
        match self.selection {
            Selection::Fixed(range) => range,
            Selection::Auto(_) => self.selector.current(),
        }
    }

    pub fn gate_is_open(&self) -> bool {
        self.gate.is_open()
    }

    /// Take `block`, sample by sample, and hand each analysed frame to `on_event`.
    pub fn push(&mut self, block: &[f32], mut on_event: impl FnMut(Event)) {
        for &input in block {
            self.position += 1;
            let sample = self.dc.process_sample(input);
            self.gate.process(std::slice::from_ref(&sample));
            if self.runs(Path::Full)
                && let Some(full) = self.band_limit.process_sample(sample)
            {
                self.feed(Range::High, full, &mut on_event);
            }
            if self.runs(Path::Decimated)
                && let Some(decimated) = self.decimator.process_sample(sample)
            {
                self.feed(Range::Low, decimated, &mut on_event);
                self.feed(Range::Mid, decimated, &mut on_event);
            }
        }
    }

    fn runs(&self, path: Path) -> bool {
        match self.selection {
            Selection::Fixed(range) => range.path() == path,
            Selection::Auto(_) => true,
        }
    }

    fn feed(&mut self, range: Range, sample: f32, on_event: &mut impl FnMut(Event)) {
        let current = self.current();
        let frames = &mut self.frames[slot(range)];
        if !frames.push_sample(sample) || range != current {
            return;
        }
        if !self.gate.is_open() {
            self.observe(Observation::Silent);
            return;
        }
        if self.position < self.analyse_from {
            return;
        }
        let rate = self.table.spec(range).analysis_rate;
        let estimate = self.detectors[slot(range)].analyze(self.frames[slot(range)].window(), rate);
        on_event(Event {
            end: self.position,
            range,
            estimate,
        });
        self.observe(match estimate {
            Some(estimate) => Observation::Pitch(estimate.frequency_hz),
            None => Observation::Unclear,
        });
    }

    fn observe(&mut self, observation: Observation) {
        if let Selection::Auto(_) = self.selection {
            self.selector.observe(observation);
        }
    }

    /// Input samples a frame of `range` spans, filter taps included: a frame ending at `end`
    /// depends on no input before `end − span`.
    pub fn span(&self, range: Range) -> u64 {
        let spec = self.table.spec(range);
        let factor = match spec.path {
            Path::Decimated => DECIMATION,
            Path::Full => 1,
        };
        let taps = 2 * self.table.group_delay(spec.path) + 1;
        u64::try_from(spec.window * factor + taps).expect("small")
    }
}

/// `1200 · log₂(measured / true)`.
pub fn cents(measured_hz: f64, true_hz: f64) -> f64 {
    1_200.0 * (measured_hz / true_hz).log2()
}
