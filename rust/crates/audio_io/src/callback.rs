//! The one function a backend calls on the real-time thread.

use crate::StreamTimestamp;

/// What a backend tells the callback about the block it is handing over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackInfo {
    /// Frames in this block. Never more than the stream's `max_block_frames`, and not constant.
    pub frames: usize,
    /// Interleaved channels in `input`; zero when the microphone is not in use.
    pub input_channels: usize,
    /// Interleaved channels in `output`.
    pub output_channels: usize,
    /// Where this block sits on the stream clock.
    pub timestamp: StreamTimestamp,
}

/// The real-time audio callback.
///
/// Called on a thread the platform owns, at a deadline it does not move. Everything `AGENTS.md` §6
/// forbids applies to every implementation: no allocation, no lock, no syscall, no log, no panic.
///
/// `input` holds `frames * input_channels` samples and `output` holds `frames * output_channels`,
/// both interleaved. The callback must write every output sample: what the buffer holds on entry is
/// unspecified.
pub trait AudioCallback: Send + 'static {
    /// Process one block.
    fn process(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo);
}
