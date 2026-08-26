//! The flutter_rust_bridge API surface.
//!
//! **This crate contains no logic** (`AGENTS.md` §5). Everything here is type mapping over
//! `diapason_engine`; business rules live in `engine` and `dsp`, where `cargo test` reaches them in
//! milliseconds without a device.

use diapason_engine::EngineSnapshot;

/// A snapshot of the engine, as seen from Dart.
///
/// Mirrors [`diapason_engine::EngineSnapshot`] rather than re-exporting it, because the mapping
/// across the boundary is this crate's whole job and the engine type must stay free to change
/// shape without the FFI layer silently following it.
pub struct EngineStatus {
    /// Identifies the DSP crate at the bottom of the Rust stack.
    pub dsp_build: String,
    /// Identifies the engine crate.
    pub engine_build: String,
    /// Whether an audio stream is running. Always `false` until `T-002`.
    pub running: bool,
}

impl From<EngineSnapshot> for EngineStatus {
    fn from(snapshot: EngineSnapshot) -> Self {
        Self {
            dsp_build: snapshot.dsp_build,
            engine_build: snapshot.engine_build,
            running: snapshot.running,
        }
    }
}

/// Read the engine's current status.
///
/// Synchronous because it reads an in-memory snapshot and returns immediately. Once the engine owns
/// a real stream this becomes a subscription to ~30 Hz snapshots rather than a poll.
#[flutter_rust_bridge::frb(sync)]
#[must_use]
pub fn engine_status() -> EngineStatus {
    EngineSnapshot::current().into()
}

/// Initialise default utilities. Called once by the generated `RustLib.init()`.
#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
