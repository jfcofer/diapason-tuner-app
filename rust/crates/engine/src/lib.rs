//! The real-time graph, command queue, snapshot publication and engine state machine.
//!
//! Dart never sees a sample (`docs/adr/0001`). It sends commands and receives ~30 Hz snapshots.
//! `T-001` ships only the snapshot type and a constructor for it, so that the value crossing the
//! FFI boundary has the same shape the real engine will publish.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

/// A point-in-time view of the engine, published for the UI to render.
///
/// Latest-wins: the UI reads the most recent snapshot and never queues them. In `T-001` it carries
/// only build identification; pitch, cents deviation and transport state arrive with the pipeline
/// that produces them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSnapshot {
    /// Identifies the DSP crate this engine was built against.
    pub dsp_build: String,
    /// Identifies the engine crate itself.
    pub engine_build: String,
    /// Whether an audio stream is currently running. Always `false` until `T-002`.
    pub running: bool,
}

impl EngineSnapshot {
    /// Build a snapshot describing the current, stopped engine.
    #[must_use]
    pub fn current() -> Self {
        Self {
            dsp_build: diapason_dsp::build_id(),
            engine_build: format!("diapason_engine {}", env!("CARGO_PKG_VERSION")),
            running: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reports_the_dsp_build_beneath_it() {
        let snapshot = EngineSnapshot::current();
        assert_eq!(snapshot.dsp_build, diapason_dsp::build_id());
        assert!(!snapshot.running, "T-001 ships no audio stream");
    }
}
