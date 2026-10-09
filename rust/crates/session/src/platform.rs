//! Which backend the app's session runs on, per target. A choice of policy, so it lives here rather
//! than in `diapason_ffi`, which contains no logic (`AGENTS.md` §5).

use crate::{Session, SessionError};

/// Spawn the session the app uses on this platform.
///
/// - Android: `AAudioBackend`, with the `VoiceRecognition` preset until `T-002b` part 2b chooses
///   the preset from the device's capabilities (`docs/PLATFORM_AUDIO.md` §2).
/// - Anything else has no backend yet (iOS arrives in `T-002c`). The session starts, and every
///   `start` fails at once with [`crate::Fault::ConfigurationUnsupported`], which the UI shows
///   rather than crashing.
///
/// # Errors
/// [`SessionError::Spawn`] if the session thread cannot be created.
pub fn spawn_platform_session() -> Result<Session, SessionError> {
    Session::spawn(backend())
}

#[cfg(target_os = "android")]
fn backend() -> diapason_audio_io::AAudioBackend {
    diapason_audio_io::AAudioBackend::new(diapason_audio_io::InputPreset::VoiceRecognition)
}

#[cfg(not(target_os = "android"))]
fn backend() -> unavailable::Unavailable {
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
