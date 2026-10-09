//! The real-time graph, command queue, snapshot publication and engine state machine.
//!
//! Dart never sees a sample (`docs/adr/0001`). The engine splits into two halves at
//! [`Engine::prepare`]:
//!
//! - [`Processor`] runs on the audio thread, inside a backend's callback.
//! - [`Engine`] stays on the control side and sends [`Command`]s and reads [`EngineSnapshot`]s.
//!
//! The halves share exactly two lock-free primitives: an `rtrb` SPSC queue for commands and a
//! `triple_buffer` for snapshots. No other cross-thread primitive is permitted in this crate
//! (`docs/AUDIO_ENGINE.md` §7).
//!
//! ```
//! use diapason_engine::{Command, Engine};
//!
//! let (mut engine, processor) = Engine::prepare(256, 48_000)?;
//! engine.send(Command::StartTone { frequency_hz: 440.0, amplitude: 0.5 })?;
//! // `processor` goes to a backend: `backend.open(config, Box::new(processor))`.
//! # drop(processor);
//! assert_eq!(engine.snapshot().sample_rate, 48_000);
//! # Ok::<(), diapason_engine::EngineError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

mod command;
mod processor;
mod snapshot;

pub use command::Command;
pub use processor::Processor;
pub use snapshot::EngineSnapshot;

use diapason_audio_io::{MAX_BLOCK_FRAMES, SAMPLE_RATES};
use diapason_dsp::level::RmsMeter;
use diapason_dsp::osc::SineOscillator;
use thiserror::Error;

/// Commands that can wait for the audio thread at once. At one callback every few milliseconds the
/// queue drains far faster than any UI can fill it; a full queue means the audio thread is not
/// running, which the sender needs to hear about rather than block on.
pub const COMMAND_CAPACITY: usize = 64;

/// Input-meter windows per second: 50 ms, fast enough to look live at ~30 Hz.
const METER_WINDOWS_PER_SECOND: u32 = 20;

/// Samples in one input-meter window at `sample_rate`.
pub(crate) fn meter_window(sample_rate: u32) -> u32 {
    sample_rate / METER_WINDOWS_PER_SECOND
}

/// Why the control side could not do what it asked.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum EngineError {
    /// [`Engine::prepare`] was given a size or rate no stream can have.
    #[error("invalid engine configuration: {0}")]
    InvalidConfig(&'static str),
    /// The command queue is full: the audio thread has stopped draining it. Never blocks.
    #[error("command queue full; is the audio stream running?")]
    QueueFull,
}

/// The control-side half of the engine.
pub struct Engine {
    commands: rtrb::Producer<Command>,
    snapshots: triple_buffer::Output<EngineSnapshot>,
}

impl Engine {
    /// Build both halves of the engine for blocks of up to `max_block_size` frames at
    /// `sample_rate`. Every buffer the [`Processor`] will ever touch is allocated here, on the
    /// calling thread, so the audio thread never has to.
    ///
    /// # Errors
    /// [`EngineError::InvalidConfig`] if `max_block_size` is outside `1..=MAX_BLOCK_FRAMES` or
    /// `sample_rate` is outside [`SAMPLE_RATES`]. The rate is only where the engine starts: if the
    /// device grants another, the [`Processor`] follows it on the first callback.
    pub fn prepare(
        max_block_size: usize,
        sample_rate: u32,
    ) -> Result<(Self, Processor), EngineError> {
        if !(1..=MAX_BLOCK_FRAMES).contains(&max_block_size) {
            return Err(EngineError::InvalidConfig("max block size outside 1–8192"));
        }
        if !SAMPLE_RATES.contains(&sample_rate) {
            return Err(EngineError::InvalidConfig("sample rate outside 8–384 kHz"));
        }

        let (producer, consumer) = rtrb::RingBuffer::new(COMMAND_CAPACITY);
        let snapshot = EngineSnapshot {
            sample_rate,
            ..EngineSnapshot::default()
        };
        let (input, output) = triple_buffer::triple_buffer(&snapshot);
        let processor = Processor {
            commands: consumer,
            snapshots: input,
            snapshot,
            tone: SineOscillator::new(sample_rate),
            tone_request: None,
            meter: RmsMeter::new(meter_window(sample_rate)),
            mono: vec![0.0; max_block_size].into_boxed_slice(),
        };
        Ok((
            Self {
                commands: producer,
                snapshots: output,
            },
            processor,
        ))
    }

    /// Queue a command for the audio thread. Returns at once, whether or not audio is running.
    ///
    /// # Errors
    /// [`EngineError::QueueFull`] if [`COMMAND_CAPACITY`] commands are already waiting.
    pub fn send(&mut self, command: Command) -> Result<(), EngineError> {
        self.commands
            .push(command)
            .map_err(|_| EngineError::QueueFull)
    }

    /// The most recent snapshot the audio thread published. Never blocks.
    #[must_use]
    pub fn snapshot(&mut self) -> EngineSnapshot {
        *self.snapshots.read()
    }
}

/// Which builds of the Rust stack are running, for the about screen and bug reports.
///
/// Off the real-time path: it owns `String`s, so it is never part of an [`EngineSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// Identifies the DSP crate this engine was built against.
    pub dsp_build: String,
    /// Identifies the engine crate itself.
    pub engine_build: String,
}

impl BuildInfo {
    /// The builds linked into this binary.
    #[must_use]
    pub fn current() -> Self {
        Self {
            dsp_build: diapason_dsp::build_id(),
            engine_build: concat!("diapason_engine ", env!("CARGO_PKG_VERSION")).to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_info_reports_the_dsp_build_beneath_it() {
        let info = BuildInfo::current();
        assert_eq!(info.dsp_build, diapason_dsp::build_id());
        assert!(info.engine_build.starts_with("diapason_engine "));
    }

    #[test]
    fn prepare_rejects_sizes_no_stream_can_have() {
        assert!(matches!(
            Engine::prepare(0, 48_000),
            Err(EngineError::InvalidConfig(_))
        ));
        assert!(matches!(
            Engine::prepare(MAX_BLOCK_FRAMES + 1, 48_000),
            Err(EngineError::InvalidConfig(_))
        ));
        assert!(matches!(
            Engine::prepare(256, 7_999),
            Err(EngineError::InvalidConfig(_))
        ));
    }

    #[test]
    fn a_full_queue_is_reported_not_waited_on() {
        let (mut engine, _processor) = Engine::prepare(256, 48_000).expect("prepare");
        for _ in 0..COMMAND_CAPACITY {
            engine.send(Command::StopTone).expect("queue has room");
        }
        assert_eq!(engine.send(Command::StopTone), Err(EngineError::QueueFull));
    }
}
