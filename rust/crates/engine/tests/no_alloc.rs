//! The audio callback never allocates: ten seconds through `OfflineBackend` with allocation
//! trapping armed (`docs/AUDIO_ENGINE.md` §7, `docs/TESTING.md` §2).
//!
//! `assert_no_alloc` aborts the process on a trapped allocation; it cannot panic, because
//! panicking allocates. So the canary that proves the trap is armed runs in a child process: this
//! same test binary, filtered to one ignored test, with an environment variable choosing whether
//! its callback allocates.

// The trapping allocator only exists in debug builds; release builds compile the check out.
#![cfg(debug_assertions)]

use std::process::{Command as Process, ExitStatus, Output};

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use diapason_audio_io::{
    AudioBackend, AudioCallback, BlockPattern, CallbackInfo, OfflineBackend, StreamConfig,
};
use diapason_engine::{Command, Engine};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

const RATE: u32 = 48_000;
const MAX_BLOCK: usize = 512;
const CONFIG: StreamConfig = StreamConfig {
    sample_rate: RATE,
    max_block_frames: MAX_BLOCK,
    input_channels: 1,
    output_channels: 2,
};
/// Frames per render: 100 ms, so ten seconds is a hundred renders.
const CHUNK: usize = 4_800;
const CHILD_MODE: &str = "DIAPASON_NO_ALLOC_CHILD";

/// Render ten seconds through `callback` with the trap armed around every render, so the backend's
/// block loop is checked as well as the callback. `between` runs before each render, unarmed.
fn ten_seconds(callback: Box<dyn AudioCallback>, mut between: impl FnMut(usize)) -> u64 {
    let mut backend = OfflineBackend::new(BlockPattern::Cycle(vec![1, 17, 96, 192, 511, 512, 3]));
    let handle = backend.open(CONFIG, callback).expect("open");
    let input = vec![0.1; CHUNK];
    let mut output = vec![0.0; CHUNK * CONFIG.output_channels];
    for chunk in 0..100 {
        between(chunk);
        assert_no_alloc(|| backend.render(&input, &mut output)).expect("render");
    }
    handle.frames()
}

#[test]
fn ten_seconds_of_audio_never_allocate() {
    // Prepared for 44.1 kHz against a 48 kHz stream, so the first callback also follows the
    // granted rate with the trap armed.
    let (mut engine, processor) = Engine::prepare(MAX_BLOCK, 44_100).expect("prepare");
    let frames = ten_seconds(Box::new(processor), |chunk| {
        let command = match chunk {
            10 => Command::StartTone {
                frequency_hz: 440.0,
                amplitude: 0.5,
            },
            50 => Command::StartTone {
                frequency_hz: 880.0,
                amplitude: 0.8,
            },
            80 => Command::StopTone,
            _ => return,
        };
        engine.send(command).expect("send");
    });

    assert_eq!(frames, 480_000);
    let snapshot = engine.snapshot();
    assert_eq!((snapshot.frames, snapshot.commands_applied), (480_000, 3));
}

/// Allocates on every callback: what the trap exists to catch.
struct Allocates;

impl AudioCallback for Allocates {
    fn process(&mut self, _: &[f32], output: &mut [f32], _: &CallbackInfo) {
        drop(std::hint::black_box(Vec::<f32>::with_capacity(16)));
        output.fill(0.0);
    }
}

#[test]
#[ignore = "runs only as the child of the_trap_is_armed"]
fn canary_child() {
    match std::env::var(CHILD_MODE).as_deref() {
        Ok("clean") => {
            let (_engine, processor) = Engine::prepare(MAX_BLOCK, RATE).expect("prepare");
            ten_seconds(Box::new(processor), |_| {});
        }
        Ok("allocate") => {
            ten_seconds(Box::new(Allocates), |_| {});
        }
        _ => {} // Not launched by the canary: nothing to prove.
    }
}

fn run_child(mode: &str) -> Output {
    let exe = std::env::current_exe().expect("test binary path");
    Process::new(exe)
        .args(["canary_child", "--exact", "--ignored", "--test-threads=1"])
        .env(CHILD_MODE, mode)
        .output()
        .expect("spawn child")
}

/// SIGABRT on Linux and macOS, where `std::process::abort` lands.
#[cfg(unix)]
fn aborted(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(6)
}

#[cfg(not(unix))]
fn aborted(status: ExitStatus) -> bool {
    !status.success()
}

#[test]
fn the_trap_is_armed() {
    // Control: the same harness with the real engine runs to completion, so the child is sound
    // and a failure below can only be the trap firing.
    let clean = run_child("clean");
    let stdout = String::from_utf8_lossy(&clean.stdout);
    assert!(clean.status.success(), "clean child failed: {stdout}");
    assert!(
        stdout.contains("1 passed"),
        "clean child did not run the canary: {stdout}"
    );

    let dirty = run_child("allocate");
    assert!(
        aborted(dirty.status),
        "an allocating callback was not trapped (status {:?})",
        dirty.status
    );
}
