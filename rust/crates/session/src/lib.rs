//! Owns the audio stream off the real-time thread: opens it, rebuilds it when the platform breaks
//! it, and publishes snapshots for the UI (`docs/adr/0022`).
//!
//! Two layers:
//!
//! - [`Supervisor`] makes every decision, and is sans-IO: it is told the time, so tests drive it
//!   through `OfflineBackend` on virtual time.
//! - [`Session`] is the thin driver around it: one thread, a control channel, the monotonic clock,
//!   and a subscriber that receives a snapshot about 30 times a second.
//!
//! Desired state is the source of truth. Each time the stream opens, the supervisor builds a
//! fresh engine and replays what was asked for, because a backend drops the old callback whenever
//! a stream closes or fails to open.
//!
//! Nothing here runs on the audio thread, so the RT lint list in `engine/clippy.toml` does not
//! apply. Panics do: release builds abort on one, so the panicking constructs are denied
//! outside tests (`docs/adr/0021`).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod capabilities;
mod driver;
mod platform;
mod supervisor;

pub use capabilities::{DeviceCapabilities, choose_input_preset};
pub use diapason_audio_io::{BackendReport, GrantedPath, InputPreset, StreamConfig};
pub use diapason_engine::{BuildInfo, EngineSnapshot};
pub use driver::{Session, SessionError};
pub use platform::spawn_platform_session;
pub use supervisor::{Fault, MicrophoneAccess, SessionSnapshot, SessionState, Supervisor};
