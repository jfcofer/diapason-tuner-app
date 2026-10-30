//! What the engine (M3) will do with the chain, and nothing more: run [`Front`] on each block as
//! the callback would, hand the block and the gate's state to [`Analysis`] as the ring buffer
//! would, and pass [`Analysis::unpitched`] back to the front as one flag. Every stage is a shipped
//! `diapason_dsp::chain` type; this file owns no signal processing.
//!
//! It allocates only in [`Tuner::new`]; `tests/no_alloc.rs` drives [`Tuner::push`] with the trap
//! armed.

use diapason_dsp::chain::{Analysis, Frame, Front, Selection};
use diapason_dsp::gate::GateConfig;
use diapason_dsp::range::{DECIMATION, Path, Range, RangeTable};

/// The longest block the callback side copies at once; longer input is cut into blocks this long.
const MAX_BLOCK: usize = 1_024;

pub struct Tuner {
    pub front: Front,
    pub analysis: Analysis,
    /// The callback's own buffer: `Front` filters in place, and the input is borrowed.
    block: Vec<f32>,
}

impl Tuner {
    pub fn new(sample_rate: u32, selection: Selection) -> Self {
        Self {
            front: Front::new(sample_rate, GateConfig::default()).expect("a supported rate"),
            analysis: Analysis::new(sample_rate, selection).expect("a supported rate"),
            block: vec![0.0; MAX_BLOCK],
        }
    }

    pub fn table(&self) -> &RangeTable {
        self.analysis.table()
    }

    /// Take `input`, in blocks of at most `MAX_BLOCK`, and hand each analysed frame to `on_frame`.
    pub fn push(&mut self, input: &[f32], mut on_frame: impl FnMut(&Frame)) {
        for chunk in input.chunks(MAX_BLOCK) {
            let block = &mut self.block[..chunk.len()];
            block.copy_from_slice(chunk);
            let gate = self.front.process(block);
            self.analysis.push(block, gate, &mut on_frame);
            self.front.set_unpitched(self.analysis.unpitched());
        }
    }

    pub fn gate_is_open(&self) -> bool {
        self.front.gate().is_open()
    }
}

/// Input samples a frame of `range` spans, filter taps included: a frame ending at `end` depends
/// on no input before `end − span`.
pub fn span(table: &RangeTable, range: Range) -> u64 {
    let spec = table.spec(range);
    let factor = match spec.path {
        Path::Decimated => DECIMATION,
        Path::Full => 1,
    };
    let taps = 2 * table.group_delay(spec.path) + 1;
    u64::try_from(spec.window * factor + taps).expect("small")
}

/// `1200 · log₂(measured / true)`.
pub fn cents(measured_hz: f64, true_hz: f64) -> f64 {
    1_200.0 * (measured_hz / true_hz).log2()
}
