//! What the audio thread publishes for everyone else to read.

use diapason_audio_io::StreamTimestamp;

/// A point-in-time view of the engine, published by the audio thread after every callback.
///
/// Latest-wins: readers take the most recent one and never queue them (`docs/ARCHITECTURE.md` §4).
/// `Copy` and fixed-size so that publishing it allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineSnapshot {
    /// The stream position at the end of the most recent block: the engine's only clock.
    pub frames: u64,
    /// Where the most recent block started, on the stream clock and the host clock. The pair the
    /// UI needs to place a frame in time (`docs/AUDIO_ENGINE.md` §6).
    pub clock: StreamTimestamp,
    /// The rate the stream is actually running at; until the first callback, the rate the engine
    /// was prepared for.
    pub sample_rate: u32,
    /// RMS of the first input channel over the most recent complete 50 ms window.
    pub input_rms: f32,
    /// The test tone's frequency in hertz while it is on; `None` once it has been stopped.
    pub tone_hz: Option<f32>,
    /// Commands applied so far, so a sender can see that one has landed.
    pub commands_applied: u64,
}
