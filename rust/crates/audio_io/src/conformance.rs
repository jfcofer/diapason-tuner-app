//! The [`AudioBackend`] contract as executable checks, shared by every backend.
//!
//! The engine relies on the contract, not on any one platform, so every backend runs this same
//! suite: `OfflineBackend` in `cargo test`, the platform backends on a device from `T-002b` on.
//! Compiled for this crate's tests, and for anyone enabling the `conformance` feature.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::{AudioBackend, AudioCallback, AudioError, CallbackInfo, StreamConfig};

/// How the suite drives one backend.
pub trait Harness {
    /// The backend under test.
    type Backend: AudioBackend;

    /// The backend, closed until the suite opens it.
    fn backend(&mut self) -> &mut Self::Backend;

    /// A valid configuration to request.
    fn config(&self) -> StreamConfig;

    /// Make at least `frames` frames pass through the open stream. An offline backend renders them;
    /// a device harness waits on the stream handle until they have played.
    fn advance(&mut self, frames: usize);
}

/// Run every check against fresh backends from `make`. Panics, naming the backend, on the first
/// broken rule.
pub fn run_all<H: Harness>(mut make: impl FnMut() -> H) {
    a_closed_backend_reports_nothing(&mut make());
    an_invalid_config_is_rejected_and_leaves_it_closed(&mut make());
    open_grants_a_valid_config_with_the_requested_channels(&mut make());
    a_second_open_is_rejected_and_the_first_stream_survives(&mut make());
    blocks_honour_the_granted_config(&mut make());
    the_stream_clock_never_runs_backwards(&mut make());
    close_drops_the_callback_and_is_idempotent(&mut make());
    a_closed_backend_reopens(&mut make());
}

/// What the probe callback saw, written from the audio thread with atomics only.
#[derive(Debug, Default)]
struct Observed {
    callbacks: AtomicU64,
    frames: AtomicU64,
    largest_block: AtomicUsize,
    empty_blocks: AtomicU64,
    bad_lengths: AtomicU64,
    input_channels: AtomicUsize,
    output_channels: AtomicUsize,
    clock_regressions: AtomicU64,
    next_frame: AtomicU64,
    last_host_ns: AtomicU64,
    dropped: AtomicBool,
}

/// A callback that writes silence and records everything it is handed.
struct Probe(Arc<Observed>);

impl AudioCallback for Probe {
    fn process(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo) {
        output.fill(0.0);
        let seen = &self.0;
        let relaxed = Ordering::Relaxed;
        if info.frames == 0 {
            seen.empty_blocks.fetch_add(1, relaxed);
        }
        if input.len() != info.frames * info.input_channels
            || output.len() != info.frames * info.output_channels
        {
            seen.bad_lengths.fetch_add(1, relaxed);
        }
        seen.input_channels.store(info.input_channels, relaxed);
        seen.output_channels.store(info.output_channels, relaxed);
        let stamp = info.timestamp;
        if stamp.frame < seen.next_frame.load(relaxed)
            || stamp.host_time_ns < seen.last_host_ns.load(relaxed)
        {
            seen.clock_regressions.fetch_add(1, relaxed);
        }
        seen.next_frame
            .store(stamp.frame + info.frames as u64, relaxed);
        seen.last_host_ns.store(stamp.host_time_ns, relaxed);
        seen.callbacks.fetch_add(1, relaxed);
        seen.frames.fetch_add(info.frames as u64, relaxed);
        seen.largest_block.fetch_max(info.frames, relaxed);
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::Relaxed);
    }
}

fn open_probe<H: Harness>(harness: &mut H) -> (Arc<Observed>, crate::StreamHandle) {
    let seen = Arc::new(Observed::default());
    let config = harness.config();
    let name = harness.backend().name();
    let handle = harness
        .backend()
        .open(config, Box::new(Probe(Arc::clone(&seen))))
        .unwrap_or_else(|error| panic!("{name}: open failed: {error}"));
    (seen, handle)
}

fn get(counter: &AtomicU64) -> u64 {
    counter.load(Ordering::Relaxed)
}

fn a_closed_backend_reports_nothing<H: Harness>(harness: &mut H) {
    let backend = harness.backend();
    let name = backend.name();
    assert_eq!(
        backend.actual_config(),
        None,
        "{name}: config reported before open"
    );
    assert_eq!(
        backend.timestamp(),
        None,
        "{name}: timestamp reported before open"
    );
    assert_eq!(
        backend.close(),
        Ok(()),
        "{name}: closing a never-opened backend must succeed"
    );
}

fn an_invalid_config_is_rejected_and_leaves_it_closed<H: Harness>(harness: &mut H) {
    let config = StreamConfig {
        output_channels: 0,
        ..harness.config()
    };
    let backend = harness.backend();
    let name = backend.name();
    let result = backend.open(config, Box::new(Probe(Arc::default())));
    assert!(
        matches!(result, Err(AudioError::InvalidConfig(_))),
        "{name}: a stream with no output must be rejected as invalid"
    );
    assert_eq!(
        backend.actual_config(),
        None,
        "{name}: a rejected open left a stream behind"
    );
}

fn open_grants_a_valid_config_with_the_requested_channels<H: Harness>(harness: &mut H) {
    let requested = harness.config();
    open_probe(harness);
    let backend = harness.backend();
    let name = backend.name();
    let granted = backend
        .actual_config()
        .unwrap_or_else(|| panic!("{name}: open but no config"));
    assert_eq!(
        granted.validate(),
        Ok(()),
        "{name}: granted an invalid config {granted:?}"
    );
    assert_eq!(
        (granted.input_channels, granted.output_channels),
        (requested.input_channels, requested.output_channels),
        "{name}: channel counts must be honoured or open must fail"
    );
    assert_eq!(
        backend.timestamp(),
        None,
        "{name}: timestamp before the first block"
    );
}

fn a_second_open_is_rejected_and_the_first_stream_survives<H: Harness>(harness: &mut H) {
    let (seen, _handle) = open_probe(harness);
    let config = harness.config();
    let backend = harness.backend();
    let name = backend.name();
    let second = backend.open(config, Box::new(Probe(Arc::default())));
    assert!(
        matches!(second, Err(AudioError::AlreadyOpen)),
        "{name}: second open was not rejected"
    );
    harness.advance(config.max_block_frames);
    assert!(
        get(&seen.callbacks) > 0,
        "{name}: the first stream stopped after a rejected open"
    );
}

fn blocks_honour_the_granted_config<H: Harness>(harness: &mut H) {
    let (seen, handle) = open_probe(harness);
    let name = harness.backend().name();
    let granted = harness
        .backend()
        .actual_config()
        .unwrap_or_else(|| panic!("{name}: no config"));
    let wanted = 10 * granted.max_block_frames + 7;
    harness.advance(wanted);

    assert!(
        get(&seen.frames) >= wanted as u64,
        "{name}: fewer frames than advanced"
    );
    assert_eq!(
        get(&seen.empty_blocks),
        0,
        "{name}: callback given an empty block"
    );
    assert_eq!(
        get(&seen.bad_lengths),
        0,
        "{name}: buffer lengths disagree with the block info"
    );
    let largest = seen.largest_block.load(Ordering::Relaxed);
    assert!(
        largest <= granted.max_block_frames,
        "{name}: block of {largest} frames exceeds the granted maximum {}",
        granted.max_block_frames
    );
    assert_eq!(
        (
            seen.input_channels.load(Ordering::Relaxed),
            seen.output_channels.load(Ordering::Relaxed)
        ),
        (granted.input_channels, granted.output_channels),
        "{name}: callback channel counts disagree with the granted config"
    );
    assert_eq!(
        handle.frames(),
        get(&seen.frames),
        "{name}: handle frame count is wrong"
    );
    assert_eq!(
        handle.callbacks(),
        get(&seen.callbacks),
        "{name}: handle callback count is wrong"
    );
    assert_eq!(
        handle.max_block_frames_seen(),
        largest,
        "{name}: handle largest block is wrong"
    );
}

fn the_stream_clock_never_runs_backwards<H: Harness>(harness: &mut H) {
    let (seen, handle) = open_probe(harness);
    let name = harness.backend().name();
    harness.advance(4096);
    assert_eq!(
        get(&seen.clock_regressions),
        0,
        "{name}: the stream clock went backwards"
    );
    let stamp = harness
        .backend()
        .timestamp()
        .unwrap_or_else(|| panic!("{name}: no timestamp"));
    assert!(
        stamp.frame < handle.frames(),
        "{name}: timestamp is ahead of the audio"
    );
}

fn close_drops_the_callback_and_is_idempotent<H: Harness>(harness: &mut H) {
    let (seen, _handle) = open_probe(harness);
    harness.advance(256);
    let backend = harness.backend();
    let name = backend.name();
    assert_eq!(backend.close(), Ok(()), "{name}: close failed");
    assert!(
        seen.dropped.load(Ordering::Relaxed),
        "{name}: close kept the callback alive"
    );
    assert_eq!(
        backend.actual_config(),
        None,
        "{name}: config reported after close"
    );
    assert_eq!(
        backend.timestamp(),
        None,
        "{name}: timestamp reported after close"
    );
    assert_eq!(
        backend.close(),
        Ok(()),
        "{name}: a second close must be a no-op"
    );
}

fn a_closed_backend_reopens<H: Harness>(harness: &mut H) {
    open_probe(harness);
    let name = harness.backend().name();
    assert_eq!(harness.backend().close(), Ok(()), "{name}: close failed");
    let (seen, _handle) = open_probe(harness);
    harness.advance(256);
    assert!(
        get(&seen.callbacks) > 0,
        "{name}: a reopened stream never ran"
    );
}
