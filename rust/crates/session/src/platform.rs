//! Which backend the app's session runs on, per target. A choice of policy, so it lives here rather
//! than in `diapason_ffi`, which contains no logic (`AGENTS.md` §5).

use crate::{DeviceCapabilities, Session, SessionError};

/// Spawn the session the app uses on this platform, given what the platform reported about the
/// device.
///
/// - Android: `AAudioBackend`, with the input preset [`crate::choose_input_preset`] picks from
///   `capabilities` (`docs/PLATFORM_AUDIO.md` §2).
/// - Anything else has no backend yet (iOS arrives in `T-002c`). The session starts, and every
///   `start` fails at once with [`crate::Fault::ConfigurationUnsupported`], which the UI shows
///   rather than crashing.
///
/// # Errors
/// [`SessionError::Spawn`] if the session thread cannot be created.
pub fn spawn_platform_session(capabilities: DeviceCapabilities) -> Result<Session, SessionError> {
    Session::spawn(backend(capabilities))
}

#[cfg(target_os = "android")]
fn backend(capabilities: DeviceCapabilities) -> diapason_audio_io::AAudioBackend {
    diapason_audio_io::AAudioBackend::new(crate::choose_input_preset(&capabilities))
}

#[cfg(not(target_os = "android"))]
fn backend(_: DeviceCapabilities) -> unavailable::Unavailable {
    unavailable::Unavailable
}

#[cfg(not(target_os = "android"))]
mod unavailable {
    use diapason_audio_io::{
        AudioBackend, AudioCallback, AudioError, Result, StreamConfig, StreamHandle,
        StreamTimestamp,
    };

    /// A placeholder for platforms with no backend yet. Not a backend: it never opens, so it is
    /// not held to the conformance suite.
    pub struct Unavailable;

    impl AudioBackend for Unavailable {
        fn name(&self) -> &'static str {
            "unavailable"
        }

        fn open(&mut self, _: StreamConfig, _: Box<dyn AudioCallback>) -> Result<StreamHandle> {
            Err(AudioError::UnsupportedConfiguration(
                "no audio backend on this platform yet".to_owned(),
            ))
        }

        fn actual_config(&self) -> Option<StreamConfig> {
            None
        }

        fn timestamp(&self) -> Option<StreamTimestamp> {
            None
        }

        fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }
}
