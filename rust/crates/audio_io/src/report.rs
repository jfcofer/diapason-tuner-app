//! What a backend can say about the path the platform actually gave it.
//!
//! Defined on every target, although only Android fills most of it in today, so that the session
//! above `audio_io` and the diagnostics overlay compile and test on any host.

/// Which processing Android applies to the microphone before the app sees it.
///
/// This decides tuner accuracy more than anything else on Android: the default source applies
/// automatic gain, noise suppression and a voice-band filter (`docs/PLATFORM_AUDIO.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPreset {
    /// No processing at all. Request it only where the device advertises
    /// `PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED`.
    Unprocessed,
    /// Turns off automatic gain on most devices. The fallback when unprocessed is not supported.
    VoiceRecognition,
    /// A preset this backend never requests, as the device reported it.
    Other(i32),
}

/// Which path one stream was actually given. Requesting low latency and exclusive mode is only a
/// request; budget devices often refuse one or both, and that refusal is where most of their
/// latency comes from (`docs/PLATFORM_AUDIO.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantedPath {
    /// The low-latency performance mode was granted.
    pub low_latency: bool,
    /// Exclusive (MMAP) sharing was granted, rather than the shared mixer.
    pub exclusive: bool,
}

/// Everything a backend knows about its open stream beyond [`crate::StreamConfig`], for the
/// diagnostics overlay and support reports. Read on the control side, never the audio thread.
///
/// Every field is `None` when the backend is closed, and wherever the platform has no such notion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackendReport {
    /// Output underruns plus input overruns the platform has counted since the stream opened.
    pub xruns: Option<u32>,
    /// The smallest block the output moves at once.
    pub frames_per_burst: Option<u32>,
    /// The input preset the backend asked for. `None` without an input.
    pub requested_input_preset: Option<InputPreset>,
    /// The input preset the device applied, which may not be the one requested. `None` without an
    /// input.
    pub input_preset: Option<InputPreset>,
    /// The path the output was given.
    pub output_path: Option<GrantedPath>,
    /// The path the input was given. `None` without an input.
    pub input_path: Option<GrantedPath>,
}
