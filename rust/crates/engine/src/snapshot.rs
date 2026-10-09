//! What the audio thread publishes for everyone else to read.

/// A point-in-time view of the engine, published by the audio thread after every callback.
///
/// Latest-wins: readers take the most recent one and never queue them (`docs/ARCHITECTURE.md` §4).
/// `Copy` and fixed-size so that publishing it allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineSnapshot {
    /// Frames processed since the engine was prepared. The engine's only clock.
    pub frames: u64,
    /// The sample rate the engine was prepared for.
    pub sample_rate: u32,
    /// RMS of the first input channel over the most recent complete 50 ms window.
    pub input_rms: f32,
    /// The test tone's frequency in hertz while it is on; `None` once it has been stopped.
    pub tone_hz: Option<f32>,
    /// Commands applied so far, so a sender can see that one has landed.
    pub commands_applied: u64,
}
