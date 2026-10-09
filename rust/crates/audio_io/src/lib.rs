//! Platform audio backends behind one trait.
//!
//! This is one of only two crates permitted to contain `unsafe` (`AGENTS.md` §7), because the
//! platform APIs it wraps require it. Every `unsafe` block carries a `// SAFETY:` comment.
//!
//! The engine knows only [`AudioBackend`] and [`AudioCallback`] (`docs/PLATFORM_AUDIO.md` §1).
//! [`OfflineBackend`] drives a stream from buffers with no device and no clock, so everything above
//! this crate is testable on any host. `AAudioBackend` is Android's (`docs/adr/0020`); Core Audio
//! arrives in `T-002c`.

#![warn(clippy::pedantic)]
#![warn(missing_docs)]

#[cfg(target_os = "android")]
mod android;
mod callback;
mod config;
#[cfg(any(test, feature = "conformance"))]
pub mod conformance;
mod handle;
mod offline;

#[cfg(target_os = "android")]
pub use android::{AAudioBackend, GrantedPath, InputPreset};
pub use callback::{AudioCallback, CallbackInfo};
pub use config::{MAX_BLOCK_FRAMES, MAX_CHANNELS, SAMPLE_RATES, StreamConfig, StreamTimestamp};
pub use handle::StreamHandle;
pub use offline::{BlockPattern, OfflineBackend};

use thiserror::Error;

/// Why a backend could not open, run or close a stream.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// The request is out of range for any backend. Names the offending field.
    #[error("invalid stream configuration: {0}")]
    InvalidConfig(&'static str),
    /// The platform refused a configuration that was valid in principle.
    #[error("unsupported stream configuration: {0}")]
    UnsupportedConfiguration(String),
    /// `open` was called on a backend that already has a stream.
    #[error("a stream is already open")]
    AlreadyOpen,
    /// The operation needs an open stream and there is none.
    #[error("no stream is open")]
    NotOpen,
    /// Buffers handed to a backend do not match the open stream's channel counts.
    #[error("buffer lengths do not match the stream: {0}")]
    BufferMismatch(&'static str),
    /// The device went away mid-stream. Oboe reports this from the callback.
    #[error("audio device disconnected")]
    Disconnected,
    /// Recording was attempted without the microphone permission having been granted.
    #[error("microphone permission not granted")]
    PermissionDenied,
    /// A platform audio call failed. Carries the platform's own code, so a support report can be
    /// looked up in its documentation. Allocates nothing to build.
    #[error("{operation} failed with platform error {code}")]
    Platform {
        /// What was being attempted, such as `"open the output stream"`.
        operation: &'static str,
        /// The platform's result code, such as an `aaudio_result_t`.
        code: i32,
    },
}

/// Result type for every backend operation.
pub type Result<T> = std::result::Result<T, AudioError>;

/// A duplex audio backend: one stream, input and output on one clock (`PLATFORM_AUDIO.md` §1).
///
/// Every implementation must pass the shared conformance suite (`src/conformance.rs`), so that the
/// engine relies on this contract rather than on any one platform's behaviour.
pub trait AudioBackend {
    /// Human-readable backend name, for the diagnostics overlay.
    fn name(&self) -> &'static str;

    /// Open a stream and start calling `callback` on the real-time thread.
    ///
    /// The callback is moved to that thread and dropped when the stream closes. Every buffer it
    /// needs must already be allocated: it may not allocate (`AGENTS.md` §6).
    ///
    /// # Errors
    /// [`AudioError::AlreadyOpen`] if a stream is open, [`AudioError::InvalidConfig`] if `config`
    /// fails [`StreamConfig::validate`], or a platform error.
    fn open(
        &mut self,
        config: StreamConfig,
        callback: Box<dyn AudioCallback>,
    ) -> Result<StreamHandle>;

    /// What the device actually granted. Channel counts always match the request (or `open` fails);
    /// the sample rate and block size may not. `None` when closed.
    fn actual_config(&self) -> Option<StreamConfig>;

    /// The stream clock at the most recent block. `None` before the first block, and when closed.
    fn timestamp(&self) -> Option<StreamTimestamp>;

    /// Stop the stream, release the device and drop the callback. Closing a closed backend is a
    /// no-op, so lifecycle code never has to track whether it already did.
    ///
    /// # Errors
    /// A platform error while tearing the stream down. The stream is closed either way.
    fn close(&mut self) -> Result<()>;
}
