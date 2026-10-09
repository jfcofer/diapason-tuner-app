//! The [`AudioBackend`] contract as executable checks, shared by every backend.
//!
//! The engine relies on the contract, not on any one platform, so every backend runs this same
//! suite: `OfflineBackend` in `cargo test`, the platform backends on a device from `T-002b` on.
//! Compiled for this crate's tests, and for anyone enabling the `conformance` feature. The suite
//! is itself tested against deliberately broken backends in `offline.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{AudioBackend, AudioCallback, AudioError, CallbackInfo, StreamConfig, StreamHandle};

/// How the suite drives one backend.
pub trait Harness {
    /// The backend under test.
    type Backend: AudioBackend;

    /// Whether the stream runs on its own thread, independently of [`advance`](Self::advance).
    /// A device harness sets this, which relaxes the checks that cannot be exact while audio keeps
    /// flowing: `timestamp()` may already be set straight after `open`, may be newer than the last
    /// block the suite observed, and the handle's counters may run ahead of the probe's.
    const REALTIME: bool = false;

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
    invalid_configs_are_rejected_and_leave_it_closed(&mut make());
    open_grants_a_valid_config_with_the_requested_channels(&mut make());
    a_second_open_is_rejected_and_the_first_stream_survives(&mut make());
    blocks_honour_the_granted_config(&mut make());
    the_stream_clock_is_contiguous_and_paired_with_a_rising_host_clock(&mut make());
    close_drops_the_callback_and_is_idempotent(&mut make());
    a_reopened_stream_starts_a_new_clock(&mut make());
}

/// What the probe callback saw, written from the audio thread with atomics only.
#[derive(Debug, Default)]
struct Observed {
    callbacks: AtomicU64,
    frames: AtomicU64,
    largest_block: AtomicUsize,
    empty_blocks: AtomicU64,
    bad_lengths: AtomicU64,
    channel_mismatches: AtomicU64,
    sample_rate: AtomicU32,
    rate_changes: AtomicU64,
    clock_breaks: AtomicU64,
    host_regressions: AtomicU64,
    next_frame: AtomicU64,
    last_frame: AtomicU64,
    last_host_ns: AtomicU64,
    dropped: AtomicBool,
}

/// A callback that writes silence and records everything it is handed.
struct Probe {
    seen: Arc<Observed>,
    channels: (usize, usize),
}

impl AudioCallback for Probe {
    fn process(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo) {
        output.fill(0.0);
        let seen = &self.seen;
        let relaxed = Ordering::Relaxed;
        let first = seen.callbacks.load(relaxed) == 0;

        if info.frames == 0 {
            seen.empty_blocks.fetch_add(1, relaxed);
        }
        if input.len() != info.frames * info.input_channels
            || output.len() != info.frames * info.output_channels
        {
            seen.bad_lengths.fetch_add(1, relaxed);
        }
        if (info.input_channels, info.output_channels) != self.channels {
            seen.channel_mismatches.fetch_add(1, relaxed);
        }
        if !first && info.sample_rate != seen.sample_rate.load(relaxed) {
            seen.rate_changes.fetch_add(1, relaxed);
        }
        seen.sample_rate.store(info.sample_rate, relaxed);

        // Contiguous from zero: the first block starts at frame 0 and each later one exactly where
        // the previous one ended. Host time rises with every block.
        let stamp = info.timestamp;
        if stamp.frame != seen.next_frame.load(relaxed) {
            seen.clock_breaks.fetch_add(1, relaxed);
        }
        if !first && stamp.host_time_ns <= seen.last_host_ns.load(relaxed) {
            seen.host_regressions.fetch_add(1, relaxed);
        }
        seen.next_frame
            .store(stamp.frame + info.frames as u64, relaxed);
        seen.last_frame.store(stamp.frame, relaxed);
        seen.last_host_ns.store(stamp.host_time_ns, relaxed);

        seen.frames.fetch_add(info.frames as u64, relaxed);
        seen.largest_block.fetch_max(info.frames, relaxed);
        seen.callbacks.fetch_add(1, relaxed);
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.seen.dropped.store(true, Ordering::Relaxed);
    }
}

fn probe_for(config: StreamConfig) -> (Arc<Observed>, Box<Probe>) {
    let seen = Arc::new(Observed::default());
    let probe = Probe {
        seen: Arc::clone(&seen),
        channels: (config.input_channels, config.output_channels),
    };
    (seen, Box::new(probe))
}

fn open_probe<H: Harness>(harness: &mut H) -> (Arc<Observed>, StreamHandle) {
    let config = harness.config();
    let (seen, probe) = probe_for(config);
    let backend = harness.backend();
    let name = backend.name();
    let handle = backend
        .open(config, probe)
        .unwrap_or_else(|error| panic!("{name}: open failed: {error}"));
    (seen, handle)
}

fn granted<H: Harness>(harness: &mut H) -> StreamConfig {
    let backend = harness.backend();
    let name = backend.name();
    backend
        .actual_config()
        .unwrap_or_else(|| panic!("{name}: a stream is open but no config is reported"))
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

fn invalid_configs_are_rejected_and_leave_it_closed<H: Harness>(harness: &mut H) {
    let valid = harness.config();
    let invalid = [
        StreamConfig {
            output_channels: 0,
            ..valid
        },
        StreamConfig {
            sample_rate: 0,
            ..valid
        },
        StreamConfig {
            max_block_frames: 0,
            ..valid
        },
    ];
    for config in invalid {
        let backend = harness.backend();
        let name = backend.name();
        let (_, probe) = probe_for(config);
        let result = backend.open(config, probe);
        assert!(
            matches!(result, Err(AudioError::InvalidConfig(_))),
            "{name}: {config:?} must be rejected as invalid, got {result:?}"
        );
        assert_eq!(
            backend.actual_config(),
            None,
            "{name}: a rejected open left a stream behind"
        );
    }
}

fn open_grants_a_valid_config_with_the_requested_channels<H: Harness>(harness: &mut H) {
    let requested = harness.config();
    open_probe(harness);
    let granted = granted(harness);
    let backend = harness.backend();
    let name = backend.name();
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
    // A real device may have run blocks before `open` even returns.
    if !H::REALTIME {
        assert_eq!(
            backend.timestamp(),
            None,
            "{name}: timestamp before the first block"
        );
    }
}

fn a_second_open_is_rejected_and_the_first_stream_survives<H: Harness>(harness: &mut H) {
    let (seen, _handle) = open_probe(harness);
    let config = harness.config();
    let backend = harness.backend();
    let name = backend.name();
    let (_, second_probe) = probe_for(config);
    let second = backend.open(config, second_probe);
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
    let granted = granted(harness);
    let name = harness.backend().name();
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
    assert_eq!(
        get(&seen.channel_mismatches),
        0,
        "{name}: callback channel counts are wrong"
    );
    assert_eq!(
        seen.sample_rate.load(Ordering::Relaxed),
        granted.sample_rate,
        "{name}: callback sample rate disagrees with the granted config"
    );
    assert_eq!(
        get(&seen.rate_changes),
        0,
        "{name}: sample rate changed mid-stream"
    );
    let largest = seen.largest_block.load(Ordering::Relaxed);
    assert!(
        largest <= granted.max_block_frames,
        "{name}: block of {largest} frames exceeds the granted maximum {}",
        granted.max_block_frames
    );
    if !H::REALTIME {
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
    }
    // Read after the probe, so on a running stream the handle can only have seen more.
    let handle_largest = handle.max_block_frames_seen();
    assert!(
        handle_largest <= granted.max_block_frames,
        "{name}: handle reports a block of {handle_largest}, above the granted maximum"
    );
    if H::REALTIME {
        assert!(
            handle_largest >= largest,
            "{name}: handle largest block {handle_largest} is behind the probe's {largest}"
        );
    } else {
        assert_eq!(
            handle_largest, largest,
            "{name}: handle largest block is wrong"
        );
    }
}

fn the_stream_clock_is_contiguous_and_paired_with_a_rising_host_clock<H: Harness>(harness: &mut H) {
    let (seen, _handle) = open_probe(harness);
    let name = harness.backend().name();
    harness.advance(4096);

    assert_eq!(
        get(&seen.clock_breaks),
        0,
        "{name}: stream clock not contiguous from zero (a block started anywhere but where the last ended)"
    );
    assert_eq!(
        get(&seen.host_regressions),
        0,
        "{name}: host time did not rise between blocks"
    );

    // Read what the callback saw before asking the backend, so a running stream can only have
    // moved the backend's answer forward.
    let last_frame = get(&seen.last_frame);
    let last_host = get(&seen.last_host_ns);
    let stamp = harness
        .backend()
        .timestamp()
        .unwrap_or_else(|| panic!("{name}: no timestamp"));
    if H::REALTIME {
        assert!(
            stamp.frame >= last_frame,
            "{name}: timestamp() is behind the audio"
        );
    } else {
        assert_eq!(
            (stamp.frame, stamp.host_time_ns),
            (last_frame, last_host),
            "{name}: timestamp() is not the stamp of the most recent block"
        );
    }
}

fn close_drops_the_callback_and_is_idempotent<H: Harness>(harness: &mut H) {
    let (seen, _handle) = open_probe(harness);
    harness.advance(256);
    let backend = harness.backend();
    let name = backend.name();
    assert_eq!(backend.close(), Ok(()), "{name}: close failed");
    // A dropped callback can no longer be called, so this also proves the stream has stopped.
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

fn a_reopened_stream_starts_a_new_clock<H: Harness>(harness: &mut H) {
    open_probe(harness);
    harness.advance(256);
    let name = harness.backend().name();
    assert_eq!(harness.backend().close(), Ok(()), "{name}: close failed");
    let (seen, _handle) = open_probe(harness);
    harness.advance(256);
    assert!(
        get(&seen.callbacks) > 0,
        "{name}: a reopened stream never ran"
    );
    assert_eq!(
        get(&seen.clock_breaks),
        0,
        "{name}: a reopened stream's clock did not restart at 0"
    );
}
