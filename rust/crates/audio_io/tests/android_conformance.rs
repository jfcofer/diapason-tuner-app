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
        // Underruns in the first second are start-up; any after it are steady state.
        let warm = handle.input_underruns();
        thread::sleep(Duration::from_secs(3));
        println!(
            "requested {preset:?}: input underruns {warm} in the first 2 s, {} in the next 3 s",
            handle.input_underruns() - warm
        );
        let config = backend.actual_config().expect("open stream has a config");
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
