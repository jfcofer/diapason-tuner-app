//! What the device says it supports, and the input preset that follows from it.
//!
//! The platform layer asks the operating system (on Android, a Kotlin channel in `core_platform`)
//! and Dart passes the answers in. Choosing a preset from them is policy, so it is decided here,
//! in Rust, and tested on any host (`docs/PLATFORM_AUDIO.md` §2, `docs/adr/0024`).

use diapason_audio_io::InputPreset;

/// What the platform reports about the audio device before any stream opens.
///
/// Each field is `None` where the platform has no such notion, or was not asked. Unknown is never
/// read as supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeviceCapabilities {
    /// The device processes nothing on the unprocessed source. Android:
    /// `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED`.
    pub unprocessed_source: Option<bool>,
    /// The device advertises a low-latency audio path. Android:
    /// `PackageManager.FEATURE_AUDIO_LOW_LATENCY`.
    pub low_latency: Option<bool>,
}

/// The input preset to request: [`InputPreset::Unprocessed`] only when the device reports both a
/// low-latency path and an unprocessed source, otherwise [`InputPreset::VoiceRecognition`], which
/// at least turns off automatic gain on most devices. Never `VoiceCommunication`, whose echo
/// cancellation and noise suppression destroy a tuner's input.
///
/// ```
/// use diapason_session::{DeviceCapabilities, InputPreset, choose_input_preset};
///
/// let capable = DeviceCapabilities { unprocessed_source: Some(true), low_latency: Some(true) };
/// assert_eq!(choose_input_preset(&capable), InputPreset::Unprocessed);
/// assert_eq!(choose_input_preset(&DeviceCapabilities::default()), InputPreset::VoiceRecognition);
/// ```
#[must_use]
pub fn choose_input_preset(capabilities: &DeviceCapabilities) -> InputPreset {
    match (capabilities.unprocessed_source, capabilities.low_latency) {
        (Some(true), Some(true)) => InputPreset::Unprocessed,
        _ => InputPreset::VoiceRecognition,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unprocessed_needs_both_capabilities_reported_and_anything_else_falls_back() {
        let answers = [None, Some(false), Some(true)];
        for unprocessed_source in answers {
            for low_latency in answers {
                let capabilities = DeviceCapabilities {
                    unprocessed_source,
                    low_latency,
                };
                let expected = if unprocessed_source == Some(true) && low_latency == Some(true) {
                    InputPreset::Unprocessed
                } else {
                    InputPreset::VoiceRecognition
                };
                assert_eq!(
                    choose_input_preset(&capabilities),
                    expected,
                    "{capabilities:?}"
                );
            }
        }
    }
}
