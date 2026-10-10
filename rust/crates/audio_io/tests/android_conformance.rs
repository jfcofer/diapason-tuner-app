//! The backend conformance suite against AAudio on a real Android device (`T-002b`).
//!
//! Built for the device and run there over adb by `just test-android-device`; on any other target
//! this file compiles to nothing. Emulators are no substitute: emulated audio says nothing about
//! a real device's callbacks (`docs/TESTING.md`).
//!
//! In debug builds the allocation trap is the global allocator, so the AAudio trampoline, which
//! runs every block inside `assert_no_alloc`, aborts the binary if it allocates on the audio thread.
//! `android_alloc_canary.rs` proves the trap is armed.

#![cfg(target_os = "android")]

use std::thread;
use std::time::{Duration, Instant};

use diapason_audio_io::conformance::{Harness, run_all};
use diapason_audio_io::{
    AAudioBackend, AudioBackend, AudioCallback, CallbackInfo, InputPreset, StreamConfig,
};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

/// How long `advance` waits for audio before declaring the stream dead.
const DEADLINE: Duration = Duration::from_secs(5);

struct Device {
    backend: AAudioBackend,
    config: StreamConfig,
}

impl Device {
    fn new(config: StreamConfig) -> Self {
        Self {
            // Every Android device has this preset, and the shell user may record with it.
            backend: AAudioBackend::new(InputPreset::VoiceRecognition),
            config,
        }
    }
}

impl Harness for Device {
    type Backend = AAudioBackend;
    const REALTIME: bool = true;

    fn backend(&mut self) -> &mut AAudioBackend {
        &mut self.backend
    }

    fn config(&self) -> StreamConfig {
        self.config
    }

    /// Wait on the stream clock, not on a timer: `frames` have played once a block starts at least
    /// that far past where the clock stood on entry.
    fn advance(&mut self, frames: usize) {
        let start = self.backend.timestamp().map_or(0, |stamp| stamp.frame);
        let target = start + frames as u64;
        let deadline = Instant::now() + DEADLINE;
        while self
            .backend
            .timestamp()
            .is_none_or(|stamp| stamp.frame < target)
        {
            assert!(
                Instant::now() < deadline,
                "aaudio: {frames} frames did not play within {DEADLINE:?}"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
}

/// The tuner's stream: microphone in, stereo out.
const DUPLEX: StreamConfig = StreamConfig {
    sample_rate: 48_000,
    max_block_frames: 1024,
    input_channels: 1,
    output_channels: 2,
};

#[test]
fn duplex_stream_passes_the_conformance_suite() {
    run_all(|| Device::new(DUPLEX));
}

#[test]
fn output_only_stream_passes_the_conformance_suite() {
    // The metronome's stream: it must work with the microphone permission denied.
    run_all(|| {
        Device::new(StreamConfig {
            input_channels: 0,
            ..DUPLEX
        })
    });
}

#[test]
fn blocks_larger_than_the_maximum_are_cut_to_fit() {
    // A maximum below any device burst forces every AAudio callback to be cut into blocks.
    run_all(|| {
        Device::new(StreamConfig {
            max_block_frames: 32,
            ..DUPLEX
        })
    });
}

#[test]
fn the_microphone_keeps_up_once_warmed_up() {
    // A backend whose every read failed would pass the contract checks, which never look at
    // input. This one does: after start-up, every block must get all of its microphone frames.
    let mut backend = AAudioBackend::new(InputPreset::VoiceRecognition);
    let handle = backend.open(DUPLEX, Box::new(Silence)).expect("open");
    thread::sleep(Duration::from_secs(2));
    let warm = handle.input_underruns();
    let shed = handle.input_frames_shed();
    let blocks = handle.callbacks();
    thread::sleep(Duration::from_secs(2));
    let short = handle.input_underruns() - warm;
    let shed_steady = handle.input_frames_shed() - shed;
    let ran = handle.callbacks() - blocks;
    backend.close().expect("close");
    // Shedding in steady state would mean the backlog check misjudges this device's delivery and
    // throws microphone audio away, which a pitch detector hears as a splice (docs/adr/0025).
    assert_eq!(
        shed_steady, 0,
        "aaudio: {shed_steady} input frames shed after warm-up, with nothing stalling"
    );
    assert!(ran > 0, "aaudio: the stream stopped running");
    assert!(
        !handle.disconnected(),
        "aaudio: the stream reported a disconnect"
    );
    assert_eq!(
        short, 0,
        "aaudio: {short} of {ran} blocks were short of input after warm-up"
    );
}

#[test]
fn a_backlog_after_a_stall_is_shed() {
    // The callback stalls once, 2 s into the stream by its own clock, for 200 ms. The microphone
    // keeps delivering, so ~9600 frames queue up behind it, which must be discarded rather than
    // kept as latency (docs/adr/0025). Shedding is counted from 1 s, after start-up, to 4 s.
    let mut backend = AAudioBackend::new(InputPreset::VoiceRecognition);
    let stall = StallOnce {
        at_seconds: 2,
        stall: Duration::from_millis(200),
        done: false,
    };
    let handle = backend.open(DUPLEX, Box::new(stall)).expect("open");
    let rate = u64::from(backend.actual_config().expect("open stream").sample_rate);
    wait_for_frame(&backend, rate);
    let before = handle.input_frames_shed();
    wait_for_frame(&backend, 4 * rate);
    let after = handle.input_frames_shed();
    backend.close().expect("close");
    println!(
        "shed: {before} frames by 1 s, {} more by 4 s",
        after - before
    );
    assert!(
        !handle.disconnected(),
        "aaudio: the stream reported a disconnect"
    );
    // At least half the stall: the rest may have been read as ordinary input, with one burst of
    // jitter on either side.
    assert!(
        after - before >= rate / 10,
        "aaudio: only {} frames shed after a 200 ms stall",
        after - before
    );
}

#[test]
fn the_output_buffer_can_be_resized_within_its_capacity() {
    let mut backend = AAudioBackend::new(InputPreset::VoiceRecognition);
    backend.open(DUPLEX, Box::new(Silence)).expect("open");
    let report = backend.report();
    let (burst, buffer, capacity) = (
        report.frames_per_burst.expect("a burst"),
        report.buffer_frames.expect("a buffer size"),
        report.buffer_capacity_frames.expect("a capacity"),
    );
    let granted = backend
        .set_buffer_frames(buffer + burst)
        .expect("a granted size");
    let reported = backend.report().buffer_frames;
    backend.close().expect("close");
    println!(
        "buffer: {buffer} at open, {granted} after asking for one burst more, capacity {capacity}"
    );
    assert!(
        (burst..=capacity).contains(&granted),
        "aaudio: granted {granted}, outside one burst ({burst}) to the capacity ({capacity})"
    );
    assert_eq!(reported, Some(granted), "the report shows the size granted");
    if capacity > buffer {
        assert!(
            granted > buffer,
            "aaudio: room to grow, but the buffer stayed at {buffer}"
        );
    } else {
        println!("buffer: already at capacity, so growth cannot help on this device");
    }
}

/// Wait until the stream clock reaches `frame`. A deadline, never a measurement.
fn wait_for_frame(backend: &AAudioBackend, frame: u64) {
    let deadline = Instant::now() + 2 * DEADLINE;
    while backend.timestamp().is_none_or(|stamp| stamp.frame < frame) {
        assert!(
            Instant::now() < deadline,
            "aaudio: frame {frame} never played"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

/// Writes silence, and once, when the stream clock passes `at_seconds`, holds the audio thread for
/// `stall`. Timed by the stream's own frames, so it fires at the same moment whatever the burst.
struct StallOnce {
    at_seconds: u64,
    stall: Duration,
    done: bool,
}

impl AudioCallback for StallOnce {
    fn process(&mut self, _: &[f32], output: &mut [f32], info: &CallbackInfo) {
        output.fill(0.0);
        if !self.done && info.timestamp.frame >= self.at_seconds * u64::from(info.sample_rate) {
            self.done = true;
            // A spin, not a sleep: allocation-free, and exactly the kind of late callback the
            // backlog check exists for.
            let until = Instant::now() + self.stall;
            while Instant::now() < until {
                std::hint::spin_loop();
            }
        }
    }
}

/// Writes silence and does nothing else, so the stream's own numbers are what gets measured.
struct Silence;

impl AudioCallback for Silence {
    fn process(&mut self, _: &[f32], output: &mut [f32], _: &CallbackInfo) {
        output.fill(0.0);
    }
}

/// Not a check: prints what this device grants, for the journal and the binding ADR.
#[test]
fn report_what_the_device_grants() {
    for preset in [InputPreset::Unprocessed, InputPreset::VoiceRecognition] {
        let mut backend = AAudioBackend::new(preset);
        let handle = backend.open(DUPLEX, Box::new(Silence)).expect("open");
        thread::sleep(Duration::from_secs(2));
        // Underruns in the first 2 s are start-up; any after them are steady state.
        let warm = handle.input_underruns();
        thread::sleep(Duration::from_secs(3));
        println!(
            "requested {preset:?}: input underruns {warm} in the first 2 s, {} in the next 3 s",
            handle.input_underruns() - warm
        );
        // What the input counters say is queued, sampled from this thread for 1 s: on the legacy
        // path a pipeline offset that no read returns, which is why shedding measures from a floor.
        let gaps: Vec<i64> = (0..100)
            .filter_map(|_| {
                thread::sleep(Duration::from_millis(10));
                backend.input_counter_gap()
            })
            .collect();
        println!(
            "requested {preset:?}: counter gap min {:?}, max {:?} frames; {} frames shed",
            gaps.iter().min(),
            gaps.iter().max(),
            handle.input_frames_shed()
        );
        let config = backend.actual_config().expect("open stream has a config");
        assert_eq!(
            backend.report().requested_input_preset,
            Some(preset),
            "the report names the preset requested"
        );
        println!(
            "requested {preset:?}: obtained {:?}, paths {:?}, {} Hz, burst {:?}, {} callbacks, \
             largest block {}, worst callback {} us, xruns {:?}, input underruns {}",
            backend.obtained_input_preset(),
            backend.granted_paths(),
            config.sample_rate,
            backend.frames_per_burst(),
            handle.callbacks(),
            handle.max_block_frames_seen(),
            handle.worst_callback_ns() / 1_000,
            backend.xruns(),
            handle.input_underruns(),
        );
        backend.close().expect("close");
    }
}
