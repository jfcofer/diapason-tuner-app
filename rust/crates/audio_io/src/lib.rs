//! Platform audio backends behind one trait.
//!
//! This is one of only two crates permitted to contain `unsafe` (`AGENTS.md` §7), because the
//! platform APIs it wraps require it. Every `unsafe` block carries a `// SAFETY:` comment. `T-001`
//! ships the trait and its error type only — no backend, no stream, no microphone.

#![warn(clippy::pedantic)]
#![warn(missing_docs)]

use thiserror::Error;

/// Why a backend could not start or continue.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The platform refused the requested configuration.
    #[error("unsupported stream configuration: {0}")]
    UnsupportedConfiguration(String),
    /// The device went away mid-stream. Oboe reports this from the callback.
    #[error("audio device disconnected")]
    Disconnected,
    /// Recording was attempted without the microphone permission having been granted.
    #[error("microphone permission not granted")]
    PermissionDenied,
}

/// A duplex audio backend.
///
/// Implementations are added in `T-002`: Oboe on Android, Core Audio on iOS, and an offline backend
/// that drives the engine from fixture buffers so the whole pipeline is testable without hardware.
pub trait AudioBackend {
    /// Human-readable backend name, for the diagnostics overlay.
    fn name(&self) -> &'static str;

    /// Start the stream. Every buffer the real-time path needs must be preallocated before this
    /// returns — the callback thread may not allocate (`AGENTS.md` §6).
    ///
    /// # Errors
    /// Returns [`AudioError`] if the platform rejects the configuration or the microphone
    /// permission has not been granted.
    fn start(&mut self) -> Result<(), AudioError>;

    /// Stop the stream and release the device.
    ///
    /// # Errors
    /// Returns [`AudioError`] if the platform reports a failure while tearing the stream down.
    fn stop(&mut self) -> Result<(), AudioError>;
}
