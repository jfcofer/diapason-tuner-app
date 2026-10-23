//! Analysis frames: a contiguous window of the most recent samples, once every hop, whatever size
//! the blocks arrive in (`docs/AUDIO_ENGINE.md` §2).

use std::fmt;

/// Why a [`FrameAssembler`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The window is empty.
    Window(usize),
    /// The hop is zero or longer than the window, which would skip samples.
    Hop {
        /// The requested hop.
        hop: usize,
        /// The window it was asked for.
        window: usize,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(window) => write!(f, "window of {window} samples is empty"),
            Self::Hop { hop, window } => {
                write!(f, "hop of {hop} samples is not in 1..={window}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Keeps the newest `window` samples of a stream and says when a hop has passed.
///
/// Frames sit on a grid fixed by the stream itself: frame `k` ends after sample `k·hop` (counting
/// from 1), however the stream was cut into blocks, so the frame sequence is a function of the
/// samples alone and its timing is the audio clock's ([`FrameAssembler::position`]). Before a whole
/// window has arrived, the oldest part of the window is the zeros the history starts with, as if
/// the stream had been silent before it began.
///
/// The history is a ring written twice, at `i` and at `i + window`, so the window is always one
/// contiguous slice and never needs copying. Allocation-free and panic-free after
/// [`FrameAssembler::new`].
///
/// ```
/// use diapason_dsp::frame::FrameAssembler;
///
/// let mut frames = FrameAssembler::new(4, 2)?;
/// let (mut latest, mut ends) = ([0.0; 4], [0; 2]);
/// let mut count = 0;
/// for block in [&[1.0, 2.0, 3.0][..], &[4.0, 5.0]] {
///     frames.push(block, |window, end| {
///         latest.copy_from_slice(window);
///         ends[count] = end;
///         count += 1;
///     });
/// }
/// assert_eq!(ends, [2, 4]); // one frame per hop, on the stream's own clock
/// assert_eq!(latest, [1.0, 2.0, 3.0, 4.0]);
/// # Ok::<(), diapason_dsp::frame::FrameError>(())
/// ```
#[derive(Debug, Clone)]
pub struct FrameAssembler {
    window: usize,
    hop: usize,
    /// `2·window` samples; the newest `window` are `history[next..next + window]`, oldest first.
    history: Vec<f32>,
    next: usize,
    since_frame: usize,
    position: u64,
}

impl FrameAssembler {
    /// An assembler yielding the newest `window` samples every `hop` samples.
    ///
    /// # Errors
    ///
    /// [`FrameError`] for an empty window, or a hop outside `1..=window`.
    pub fn new(window: usize, hop: usize) -> Result<Self, FrameError> {
        if window == 0 {
            return Err(FrameError::Window(window));
        }
        if !(1..=window).contains(&hop) {
            return Err(FrameError::Hop { hop, window });
        }
        Ok(Self {
            window,
            hop,
            history: vec![0.0; 2 * window],
            next: 0,
            since_frame: 0,
            position: 0,
        })
    }

    /// Samples per frame.
    #[must_use]
    pub fn window_len(&self) -> usize {
        self.window
    }

    /// Samples between frames.
    #[must_use]
    pub fn hop(&self) -> usize {
        self.hop
    }

    /// Samples taken so far: the stream position just after the newest one.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// The newest `window` samples, oldest first.
    #[must_use]
    pub fn window(&self) -> &[f32] {
        self.history
            .get(self.next..self.next + self.window)
            .unwrap_or_default()
    }

    /// Take one sample; `true` if it completes a hop, so that [`Self::window`] is a new frame.
    #[inline]
    pub fn push_sample(&mut self, sample: f32) -> bool {
        if let Some(slot) = self.history.get_mut(self.next) {
            *slot = sample;
        }
        if let Some(slot) = self.history.get_mut(self.next + self.window) {
            *slot = sample;
        }
        self.next += 1;
        if self.next == self.window {
            self.next = 0;
        }
        self.position += 1;
        self.since_frame += 1;
        if self.since_frame < self.hop {
            return false;
        }
        self.since_frame = 0;
        true
    }

    /// Take every sample of `samples`, in order, and hand each completed frame to `on_frame`, with
    /// the stream position just after its newest sample.
    pub fn push(&mut self, samples: &[f32], mut on_frame: impl FnMut(&[f32], u64)) {
        for &sample in samples {
            if self.push_sample(sample) {
                on_frame(self.window(), self.position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn invalid_shapes_are_rejected() {
        assert_eq!(
            FrameAssembler::new(0, 1).unwrap_err(),
            FrameError::Window(0)
        );
        assert_eq!(
            FrameAssembler::new(8, 0).unwrap_err(),
            FrameError::Hop { hop: 0, window: 8 }
        );
        assert_eq!(
            FrameAssembler::new(8, 9).unwrap_err(),
            FrameError::Hop { hop: 9, window: 8 }
        );
        assert!(FrameAssembler::new(8, 8).is_ok());
    }

    proptest! {
        /// For any block split, frames come every hop, each the `window` samples ending there.
        #[test]
        fn every_hop_yields_the_window_ending_there_for_any_block_split(
            blocks in prop::collection::vec(0_usize..700, 1..30),
            window in 1_usize..300,
            hop_fraction in 0.0_f64..1.0,
        ) {
            let span = crate::convert::index_to_f64(window - 1);
            let hop = 1 + crate::convert::index_from_f64(hop_fraction * span);
            let total: usize = blocks.iter().sum();
            let stream: Vec<f32> = (0..total)
                .map(|i| f32::from(u16::try_from(i % 60_000).unwrap()))
                .collect();
            let mut frames = FrameAssembler::new(window, hop).unwrap();
            let mut count = 0;
            let mut start = 0;
            for size in blocks {
                frames.push(&stream[start..start + size], |frame, end| {
                    count += 1;
                    let end = usize::try_from(end).unwrap();
                    assert_eq!(end, count * hop);
                    for (k, &sample) in frame.iter().enumerate() {
                        // Before the stream began, the history is zeros.
                        let expected = (end + k).checked_sub(window).map_or(0.0, |i| stream[i]);
                        assert_eq!(sample.to_bits(), expected.to_bits());
                    }
                });
                start += size;
            }
            assert_eq!(count, total / hop);
            assert_eq!(frames.position(), u64::try_from(total).unwrap());
        }
    }
}
