//! A lock-free view of a running stream, for whoever is not on the audio thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

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
    worst_callback_ns: AtomicU64,
    input_underruns: AtomicU64,
    disconnected: AtomicBool,
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

    /// The longest a platform callback has taken, in nanoseconds, including every block it was cut
    /// into. Zero for backends with no real-time clock (`OfflineBackend`).
    #[must_use]
    pub fn worst_callback_ns(&self) -> u64 {
        self.stats.worst_callback_ns.load(Ordering::Relaxed)
    }

    /// Blocks whose input the microphone had not yet delivered, so the callback was handed silence
    /// for the missing frames. A duplex stream built from two platform streams can drift apart;
    /// this is how that shows.
    #[must_use]
    pub fn input_underruns(&self) -> u64 {
        self.stats.input_underruns.load(Ordering::Relaxed)
    }

    /// Whether the platform reported that the stream broke, for example because its device went
    /// away. The stream does not recover by itself: whoever owns the backend closes and reopens it,
    /// off the real-time thread (`docs/PLATFORM_AUDIO.md` §2).
    #[must_use]
    pub fn disconnected(&self) -> bool {
        self.stats.disconnected.load(Ordering::Relaxed)
    }

    /// Record how long one platform callback took. Real-time thread: atomics only.
    #[cfg(target_os = "android")]
    pub(crate) fn record_duration(&self, nanos: u64) {
        self.stats
            .worst_callback_ns
            .fetch_max(nanos, Ordering::Relaxed);
    }

    /// Record a block that was short of input. Real-time thread: atomics only.
    #[cfg(target_os = "android")]
    pub(crate) fn record_input_underrun(&self) {
        self.stats.input_underruns.fetch_add(1, Ordering::Relaxed);
    }

    /// Record that the platform broke the stream. Called from the platform's error thread, or by
    /// [`crate::OfflineBackend::simulate_disconnect`].
    pub(crate) fn mark_disconnected(&self) {
        self.stats.disconnected.store(true, Ordering::Relaxed);
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
