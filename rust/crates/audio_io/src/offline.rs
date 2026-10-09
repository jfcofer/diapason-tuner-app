//! A backend with no device and no clock: the caller pushes buffers through the callback.

use crate::{
    AudioBackend, AudioCallback, AudioError, CallbackInfo, Result, StreamConfig, StreamHandle,
    StreamTimestamp,
};

/// How [`OfflineBackend`] cuts a render into callback blocks.
///
/// Real devices do not promise a constant block size; some Android devices alternate between two.
/// A cycled pattern of awkward sizes is how tests catch code that quietly assumes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockPattern {
    /// Every block has this many frames.
    Fixed(usize),
    /// Block sizes are taken from the list in order, wrapping around.
    Cycle(Vec<usize>),
}

impl BlockPattern {
    /// The `index`th block size, before clamping to the stream's maximum. Never zero.
    fn size(&self, index: usize) -> usize {
        let size = match self {
            Self::Fixed(size) => *size,
            Self::Cycle(sizes) if sizes.is_empty() => 1,
            Self::Cycle(sizes) => sizes[index % sizes.len()],
        };
        size.max(1)
    }
}

/// A deterministic, faster-than-real-time backend for tests (`docs/PLATFORM_AUDIO.md` §1).
///
/// Nothing runs until [`render`](Self::render) is called, and then the callback runs on the
/// caller's thread. The stream clock advances by exactly the frames rendered, and the host clock is
/// derived from it, so the same input always produces the same output and the same timestamps.
///
/// ```
/// use diapason_audio_io::{AudioBackend, AudioCallback, BlockPattern, CallbackInfo, OfflineBackend, StreamConfig};
///
/// struct Passthrough;
/// impl AudioCallback for Passthrough {
///     fn process(&mut self, input: &[f32], output: &mut [f32], _: &CallbackInfo) {
///         output.copy_from_slice(input);
///     }
/// }
///
/// let config = StreamConfig { sample_rate: 48_000, max_block_frames: 64, input_channels: 1, output_channels: 1 };
/// let mut backend = OfflineBackend::new(BlockPattern::Fixed(64));
/// let handle = backend.open(config, Box::new(Passthrough))?;
///
/// let input = [0.25_f32; 100];
/// let mut output = [0.0_f32; 100];
/// backend.render(&input, &mut output)?;
/// assert_eq!(output, input);
/// assert_eq!(handle.callbacks(), 2); // 64 + 36 frames
/// # Ok::<(), diapason_audio_io::AudioError>(())
/// ```
pub struct OfflineBackend {
    pattern: BlockPattern,
    stream: Option<Stream>,
}

struct Stream {
    config: StreamConfig,
    callback: Box<dyn AudioCallback>,
    handle: StreamHandle,
    next_frame: u64,
    blocks: usize,
    last: Option<StreamTimestamp>,
}

impl OfflineBackend {
    /// A closed backend that will cut renders according to `pattern`.
    #[must_use]
    pub fn new(pattern: BlockPattern) -> Self {
        Self {
            pattern,
            stream: None,
        }
    }

    /// Run the open stream over `input`, writing `output`.
    ///
    /// Both are interleaved with the stream's channel counts and must hold the same number of
    /// frames. They are cut into blocks by the [`BlockPattern`], each clamped to the stream's
    /// `max_block_frames`, and handed to the callback in order. Allocates nothing.
    ///
    /// # Errors
    /// [`AudioError::NotOpen`] without a stream, or [`AudioError::BufferMismatch`] if the buffer
    /// lengths do not describe the same number of whole frames.
    pub fn render(&mut self, input: &[f32], output: &mut [f32]) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(AudioError::NotOpen)?;
        let StreamConfig {
            input_channels,
            output_channels,
            ..
        } = stream.config;

        if !output.len().is_multiple_of(output_channels) {
            return Err(AudioError::BufferMismatch(
                "output is not a whole number of frames",
            ));
        }
        let frames = output.len() / output_channels;
        if input.len() != frames * input_channels {
            return Err(AudioError::BufferMismatch(
                "input and output hold different frame counts",
            ));
        }

        let mut done = 0;
        while done < frames {
            let block = self
                .pattern
                .size(stream.blocks)
                .min(stream.config.max_block_frames)
                .min(frames - done);
            let timestamp = StreamTimestamp {
                frame: stream.next_frame,
                host_time_ns: host_time_ns(stream.next_frame, stream.config.sample_rate),
            };
            let info = CallbackInfo {
                frames: block,
                input_channels,
                output_channels,
                timestamp,
            };
            stream.callback.process(
                &input[done * input_channels..(done + block) * input_channels],
                &mut output[done * output_channels..(done + block) * output_channels],
                &info,
            );
            stream.handle.record(block);
            stream.last = Some(timestamp);
            stream.next_frame += block as u64;
            stream.blocks = stream.blocks.wrapping_add(1);
            done += block;
        }
        Ok(())
    }
}

/// The synthetic host clock: exactly where `frame` falls at `sample_rate`, from a zero epoch.
fn host_time_ns(frame: u64, sample_rate: u32) -> u64 {
    let ns = u128::from(frame) * 1_000_000_000 / u128::from(sample_rate);
    u64::try_from(ns).unwrap_or(u64::MAX)
}

impl AudioBackend for OfflineBackend {
    fn name(&self) -> &'static str {
        "offline"
    }

    fn open(
        &mut self,
        config: StreamConfig,
        callback: Box<dyn AudioCallback>,
    ) -> Result<StreamHandle> {
        if self.stream.is_some() {
            return Err(AudioError::AlreadyOpen);
        }
        config.validate()?;
        let handle = StreamHandle::new();
        self.stream = Some(Stream {
            config,
            callback,
            handle: handle.clone(),
            next_frame: 0,
            blocks: 0,
            last: None,
        });
        Ok(handle)
    }

    fn actual_config(&self) -> Option<StreamConfig> {
        self.stream.as_ref().map(|stream| stream.config)
    }

    fn timestamp(&self) -> Option<StreamTimestamp> {
        self.stream.as_ref().and_then(|stream| stream.last)
    }

    fn close(&mut self) -> Result<()> {
        self.stream = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: StreamConfig = StreamConfig {
        sample_rate: 48_000,
        max_block_frames: 512,
        input_channels: 1,
        output_channels: 2,
    };

    /// Writes each output frame as its own index on the stream clock, so block cuts are visible.
    struct FrameIndex;
    impl AudioCallback for FrameIndex {
        fn process(&mut self, _: &[f32], output: &mut [f32], info: &CallbackInfo) {
            for (offset, frame) in output.chunks_exact_mut(info.output_channels).enumerate() {
                let index = info.timestamp.frame + offset as u64;
                frame.fill(u16::try_from(index).map_or(f32::NAN, f32::from));
            }
        }
    }

    #[test]
    fn render_is_contiguous_across_irregular_blocks() {
        let mut backend = OfflineBackend::new(BlockPattern::Cycle(vec![1, 17, 96, 1024]));
        let handle = backend.open(CONFIG, Box::new(FrameIndex)).expect("open");
        let input = vec![0.0; 3000];
        let mut output = vec![0.0; 6000];
        backend.render(&input, &mut output).expect("render");

        let expected: Vec<f32> = (0..3000_u16).flat_map(|i| [f32::from(i); 2]).collect();
        assert_eq!(output, expected);
        assert_eq!(handle.frames(), 3000);
        assert_eq!(
            handle.max_block_frames_seen(),
            512,
            "1024 is clamped to the stream maximum"
        );
    }

    struct OfflineHarness {
        backend: OfflineBackend,
    }

    impl crate::conformance::Harness for OfflineHarness {
        type Backend = OfflineBackend;

        fn backend(&mut self) -> &mut OfflineBackend {
            &mut self.backend
        }

        fn config(&self) -> StreamConfig {
            CONFIG
        }

        fn advance(&mut self, frames: usize) {
            let input = vec![0.0; frames * CONFIG.input_channels];
            let mut output = vec![0.0; frames * CONFIG.output_channels];
            self.backend.render(&input, &mut output).expect("render");
        }
    }

    #[test]
    fn passes_the_conformance_suite_with_fixed_blocks() {
        crate::conformance::run_all(|| OfflineHarness {
            backend: OfflineBackend::new(BlockPattern::Fixed(256)),
        });
    }

    #[test]
    fn passes_the_conformance_suite_with_irregular_blocks() {
        crate::conformance::run_all(|| OfflineHarness {
            backend: OfflineBackend::new(BlockPattern::Cycle(vec![1, 17, 96, 511, 2048])),
        });
    }

    #[test]
    fn host_time_follows_the_stream_clock_exactly() {
        assert_eq!(host_time_ns(48_000, 48_000), 1_000_000_000);
        assert_eq!(host_time_ns(1, 44_100), 22_675);
    }

    #[test]
    fn render_rejects_mismatched_buffers_and_a_closed_stream() {
        let mut backend = OfflineBackend::new(BlockPattern::Fixed(64));
        assert_eq!(backend.render(&[], &mut []), Err(AudioError::NotOpen));
        backend.open(CONFIG, Box::new(FrameIndex)).expect("open");
        assert!(matches!(
            backend.render(&[0.0; 3], &mut [0.0; 3]),
            Err(AudioError::BufferMismatch(_))
        ));
        assert!(matches!(
            backend.render(&[0.0; 2], &mut [0.0; 2]),
            Err(AudioError::BufferMismatch(_))
        ));
    }
}
