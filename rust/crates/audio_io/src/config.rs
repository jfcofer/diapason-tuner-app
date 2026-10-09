//! What a stream is asked for, and where it is in time.

use crate::{AudioError, Result};

/// The largest block any backend may hand the callback. Buffers on the real-time path are sized
/// from this, so it bounds memory as well as latency. 8192 frames is ~170 ms at 48 kHz, far above
/// anything a low-latency device negotiates.
pub const MAX_BLOCK_FRAMES: usize = 8192;

/// The most channels a stream may carry in either direction.
pub const MAX_CHANNELS: usize = 8;

/// A stream configuration: requested by the engine, then reported back by the backend as what the
/// device actually granted (`docs/PLATFORM_AUDIO.md` §1).
///
/// Samples are `f32`, interleaved. `input_channels` may be zero: the metronome must run without the
/// microphone permission. A stream always has output.
///
/// ```
/// use diapason_audio_io::StreamConfig;
///
/// let config = StreamConfig { sample_rate: 48_000, max_block_frames: 256, input_channels: 1, output_channels: 2 };
/// assert!(config.validate().is_ok());
/// assert!(StreamConfig { output_channels: 0, ..config }.validate().is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamConfig {
    /// Frames per second.
    pub sample_rate: u32,
    /// The most frames a single callback will be given. Blocks may be smaller, and vary.
    pub max_block_frames: usize,
    /// Interleaved input channels; zero when the microphone is not in use.
    pub input_channels: usize,
    /// Interleaved output channels; at least one.
    pub output_channels: usize,
}

impl StreamConfig {
    /// Check that the configuration is one any backend could honour.
    ///
    /// # Errors
    /// [`AudioError::InvalidConfig`] naming the first field out of range.
    pub fn validate(&self) -> Result<()> {
        if !(8_000..=384_000).contains(&self.sample_rate) {
            return Err(AudioError::InvalidConfig("sample rate outside 8–384 kHz"));
        }
        if !(1..=MAX_BLOCK_FRAMES).contains(&self.max_block_frames) {
            return Err(AudioError::InvalidConfig("max block frames outside 1–8192"));
        }
        if self.input_channels > MAX_CHANNELS {
            return Err(AudioError::InvalidConfig("more than 8 input channels"));
        }
        if !(1..=MAX_CHANNELS).contains(&self.output_channels) {
            return Err(AudioError::InvalidConfig("output channels outside 1–8"));
        }
        Ok(())
    }
}

/// Where a block sits on the stream's own clock, paired with the host clock.
///
/// `frame` is the index of the block's first frame since the stream opened. It is the only clock
/// the engine trusts (`AGENTS.md` §6); `host_time_ns` exists so the UI can convert a frame into a
/// moment it can animate against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamTimestamp {
    /// Index of the first frame of the block, counted from the start of the stream.
    pub frame: u64,
    /// The host clock at that frame, in nanoseconds. Backend-defined epoch.
    pub host_time_ns: u64,
}
