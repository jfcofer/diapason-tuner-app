//! What a stream is asked for, and where it is in time.

use std::ops::RangeInclusive;

use crate::{AudioError, Result};

/// Sample rates any stream may run at. Wide on purpose: a device's native rate is used as granted
/// (`docs/PLATFORM_AUDIO.md` §2), and anything outside this is a broken report, not a device.
pub const SAMPLE_RATES: RangeInclusive<u32> = 8_000..=384_000;

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
    /// Frames per second. A device may grant a different rate from the one requested; the callback
    /// learns the rate it is actually running at from [`crate::CallbackInfo::sample_rate`].
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
        if !SAMPLE_RATES.contains(&self.sample_rate) {
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
/// `frame` is the index of the block's first frame on the stream's own clock, which starts at zero
/// when the stream opens and advances by exactly the frames delivered. A backend whose platform
/// clock starts elsewhere (Core Audio's `mSampleTime`) subtracts its origin. It is the only clock the
/// engine trusts (`AGENTS.md` §6); `host_time_ns` pairs it with the host clock, so the UI can turn a
/// frame into a moment to animate against, and strictly increases from block to block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StreamTimestamp {
    /// Index of the first frame of the block, counted from the start of the stream.
    pub frame: u64,
    /// When that frame is heard, on the host clock, in nanoseconds (`docs/AUDIO_ENGINE.md` §6).
    /// Where the platform reports presentation (AAudio's `getTimestamp`), this includes the
    /// output latency; before it does, and for `OfflineBackend`, it is when the frame is rendered.
    /// Backend-defined epoch.
    pub host_time_ns: u64,
}
