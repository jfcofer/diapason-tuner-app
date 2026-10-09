//! A lock-free view of a running stream, for whoever is not on the audio thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Counters a backend updates from the real-time thread and anyone else may read.
///
/// Returned by [`crate::AudioBackend::open`]. Cloning it is cheap and every clone sees the same
/// stream. It is how the conformance suite knows audio is flowing on a device it cannot pump, and
/// what the diagnostics overlay will read; it never blocks either side.
#[derive(Debug, Clone, Default)]
pub struct StreamHandle {
    stats: Arc<Stats>,
}

#[derive(Debug, Default)]
struct Stats {
    callbacks: AtomicU64,
    frames: AtomicU64,
    max_block_frames: AtomicUsize,
}

impl StreamHandle {
    /// A handle for a stream that has not run yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Callbacks completed since the stream opened.
    #[must_use]
    pub fn callbacks(&self) -> u64 {
        self.stats.callbacks.load(Ordering::Relaxed)
    }

    /// Frames processed since the stream opened.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.stats.frames.load(Ordering::Relaxed)
    }

    /// The largest block the callback has been given.
    #[must_use]
    pub fn max_block_frames_seen(&self) -> usize {
        self.stats.max_block_frames.load(Ordering::Relaxed)
    }

    /// Record one completed callback. Called by backends on the real-time thread: atomics only.
    pub(crate) fn record(&self, frames: usize) {
        self.stats.callbacks.fetch_add(1, Ordering::Relaxed);
        self.stats
            .frames
            .fetch_add(frames as u64, Ordering::Relaxed);
        self.stats
            .max_block_frames
            .fetch_max(frames, Ordering::Relaxed);
    }
}
