//! Proves the allocation trap is armed on the device, so a clean `android_conformance` run means
//! something (`T-002b`).
//!
//! It must **die of SIGABRT**: the callback allocates, which inside the AAudio trampoline's
//! `assert_no_alloc` aborts the process. `just test-android-device` treats a pass as a failure.
//! Debug builds only, since the trap does not exist in release.

#![cfg(all(target_os = "android", debug_assertions))]

use std::hint::black_box;
use std::thread;
use std::time::Duration;

use diapason_audio_io::{
    AAudioBackend, AudioBackend, AudioCallback, CallbackInfo, InputPreset, StreamConfig,
};

#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

struct Allocates;

impl AudioCallback for Allocates {
    fn process(&mut self, _: &[f32], output: &mut [f32], _: &CallbackInfo) {
        output.fill(0.0);
        black_box(vec![0_u8; 64]);
    }
}

#[test]
fn an_allocating_callback_aborts() {
    let mut backend = AAudioBackend::new(InputPreset::VoiceRecognition);
    let config = StreamConfig {
        sample_rate: 48_000,
        max_block_frames: 1024,
        input_channels: 0,
        output_channels: 2,
    };
    backend.open(config, Box::new(Allocates)).expect("open");
    thread::sleep(Duration::from_secs(2));
    backend.close().expect("close");
}
