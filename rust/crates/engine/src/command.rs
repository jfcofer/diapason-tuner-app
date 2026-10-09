//! What the control side may ask of the audio thread.

/// A request from the control side to the audio thread.
///
/// `Copy` and fixed-size, so it crosses the lock-free queue without allocating on either side.
/// Commands are applied at the start of the next callback, in the order they were sent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    /// Play a sine at `frequency_hz` with peak `amplitude` in `[0, 1]`, ramping in without a
    /// click. Retuning a playing tone keeps its phase.
    StartTone {
        /// Frequency in hertz, clamped to `[0, Nyquist]`.
        frequency_hz: f32,
        /// Peak amplitude, clamped to `[0, 1]`.
        amplitude: f32,
    },
    /// Fade the tone out.
    StopTone,
}
