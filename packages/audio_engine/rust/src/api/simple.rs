//! The flutter_rust_bridge API surface.
//!
//! **This crate contains no logic** (`AGENTS.md` §5). Everything here is type mapping over
//! `diapason_session`; business rules live beneath it, where `cargo test` reaches them in
//! milliseconds without a device. The stream itself is in `session.rs`.

use diapason_session::BuildInfo;

/// Which builds of the Rust stack are running, as seen from Dart.
///
/// Built from [`diapason_session::BuildInfo`] rather than re-exporting it, because the mapping
/// across the boundary is this crate's whole job and the engine type must stay free to change
/// shape without the FFI layer silently following it.
pub struct EngineStatus {
    /// Identifies the DSP crate at the bottom of the Rust stack.
    pub dsp_build: String,
    /// Identifies the engine crate.
    pub engine_build: String,
}

impl From<BuildInfo> for EngineStatus {
    fn from(info: BuildInfo) -> Self {
        Self {
            dsp_build: info.dsp_build,
            engine_build: info.engine_build,
        }
    }
}

/// Read which builds are linked. Synchronous: it reads constants. Stream state is not here; it
/// arrives as snapshots from [`crate::api::session::AudioSession::snapshots`].
#[flutter_rust_bridge::frb(sync)]
#[must_use]
pub fn engine_status() -> EngineStatus {
    BuildInfo::current().into()
}

/// Initialise default utilities. Called once by the generated `RustLib.init()`.
#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
