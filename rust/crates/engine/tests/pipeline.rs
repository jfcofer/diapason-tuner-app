//! The engine end to end through `OfflineBackend`: commands in, audio and snapshots out.

use std::f64::consts::{FRAC_1_SQRT_2, TAU};

use diapason_audio_io::{
    AudioBackend, AudioCallback, BlockPattern, CallbackInfo, OfflineBackend, StreamConfig,
    StreamTimestamp,
};
use diapason_dsp::convert::to_f32;
use diapason_engine::{Command, Engine};

const RATE: u32 = 48_000;
const MAX_BLOCK: usize = 512;
const CONFIG: StreamConfig = StreamConfig {
    sample_rate: RATE,
    max_block_frames: MAX_BLOCK,
    input_channels: 1,
    output_channels: 2,
};

/// Block sizes a real device might plausibly alternate between, plus a few hostile ones.
fn irregular() -> BlockPattern {
    BlockPattern::Cycle(vec![1, 17, 96, 192, 511, 512, 3])
}

/// A closed-form sine fixture, independent of the engine's oscillator.
fn sine(amplitude: f64, hz: f64, frames: u32) -> Vec<f32> {
    (0..frames)
        .map(|i| to_f32(amplitude * (TAU * hz * f64::from(i) / f64::from(RATE)).sin()))
        .collect()
}

/// Frequency from interpolated upward zero crossings.
fn measured_hz(samples: &[f32]) -> f64 {
    let crossings: Vec<f64> = (0_u32..)
        .zip(samples.windows(2))
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, w)| f64::from(i) + f64::from(w[0]) / f64::from(w[0] - w[1]))
        .collect();
    let (Some(first), Some(last)) = (crossings.first(), crossings.last()) else {
        return 0.0;
    };
    let cycles = u32::try_from(crossings.len() - 1).unwrap_or(0);
    f64::from(cycles) * f64::from(RATE) / (last - first)
}

/// Render `frames` of `input` through a freshly prepared engine, sending `commands` first.
fn run(pattern: BlockPattern, commands: &[Command], input: &[f32]) -> (Engine, Vec<f32>) {
    let (mut engine, processor) = Engine::prepare(MAX_BLOCK, RATE).expect("prepare");
    for &command in commands {
        engine.send(command).expect("send");
    }
    let mut backend = OfflineBackend::new(pattern);
    backend.open(CONFIG, Box::new(processor)).expect("open");
    let mut output = vec![0.0; input.len() * CONFIG.output_channels];
    backend.render(input, &mut output).expect("render");
    (engine, output)
}

fn left(interleaved: &[f32]) -> Vec<f32> {
    interleaved.iter().step_by(2).copied().collect()
}

#[test]
fn input_level_reaches_the_snapshot() {
    let amplitude = 0.5;
    let (mut engine, _) = run(irregular(), &[], &sine(amplitude, 1_000.0, 9_600));
    let snapshot = engine.snapshot();
    assert_eq!(snapshot.frames, 9_600);
    assert_eq!(snapshot.sample_rate, RATE);
    let error = (f64::from(snapshot.input_rms) - amplitude * FRAC_1_SQRT_2).abs();
    assert!(
        error < 1e-6,
        "input RMS {} is off by {error}",
        snapshot.input_rms
    );
}

#[test]
fn the_tone_is_on_pitch_and_identical_however_the_blocks_fall() {
    let start = [Command::StartTone {
        frequency_hz: 440.0,
        amplitude: 0.8,
    }];
    let silence = vec![0.0; 48_000];
    let (mut engine, irregular_out) = run(irregular(), &start, &silence);
    let (_, regular_out) = run(BlockPattern::Fixed(MAX_BLOCK), &start, &silence);

    assert_eq!(
        irregular_out, regular_out,
        "block boundaries changed the waveform"
    );
    let (frames, _) = irregular_out.as_chunks::<2>();
    let (l, r): (Vec<f32>, Vec<f32>) = frames.iter().map(|&[l, r]| (l, r)).unzip();
    assert_eq!(l, r, "every output channel carries the tone");
    let error = (measured_hz(&l) - 440.0).abs();
    assert!(error < 1e-3, "tone is {error} Hz off 440");

    let snapshot = engine.snapshot();
    assert_eq!(
        snapshot.tone_hz.map(f32::to_bits),
        Some(440.0_f32.to_bits())
    );
    assert_eq!(snapshot.commands_applied, 1);
}

#[test]
fn stopping_the_tone_fades_it_to_silence() {
    let (mut engine, processor) = Engine::prepare(MAX_BLOCK, RATE).expect("prepare");
    let mut backend = OfflineBackend::new(irregular());
    backend.open(CONFIG, Box::new(processor)).expect("open");
    let input = vec![0.0; 4_800];
    let mut output = vec![0.0; 9_600];

    engine
        .send(Command::StartTone {
            frequency_hz: 1_000.0,
            amplitude: 0.5,
        })
        .expect("send");
    backend.render(&input, &mut output).expect("render");
    assert!(output.iter().any(|&s| s != 0.0), "tone never started");

    engine.send(Command::StopTone).expect("send");
    backend.render(&input, &mut output).expect("render");
    assert!(
        left(&output)[1_000..].iter().all(|&s| s == 0.0),
        "tone did not fade out"
    );
    let snapshot = engine.snapshot();
    assert_eq!((snapshot.tone_hz, snapshot.commands_applied), (None, 2));
}

#[test]
fn the_processor_runs_on_another_thread() {
    let (mut engine, processor) = Engine::prepare(MAX_BLOCK, RATE).expect("prepare");
    engine
        .send(Command::StartTone {
            frequency_hz: 220.0,
            amplitude: 0.25,
        })
        .expect("send");
    let audio = std::thread::spawn(move || {
        let mut backend = OfflineBackend::new(irregular());
        backend.open(CONFIG, Box::new(processor)).expect("open");
        let input = vec![0.0; 48_000];
        let mut output = vec![0.0; 96_000];
        backend.render(&input, &mut output).expect("render");
    });
    audio.join().expect("audio thread");

    let snapshot = engine.snapshot();
    assert_eq!(snapshot.frames, 48_000);
    assert_eq!(snapshot.commands_applied, 1);
}

#[test]
fn an_oversized_block_is_rendered_in_prepared_slices() {
    let start = Command::StartTone {
        frequency_hz: 440.0,
        amplitude: 0.8,
    };
    let render = |prepared: usize| {
        let (mut engine, mut processor) = Engine::prepare(prepared, RATE).expect("prepare");
        engine.send(start).expect("send");
        let info = CallbackInfo {
            frames: 700,
            sample_rate: RATE,
            input_channels: 1,
            output_channels: 2,
            timestamp: StreamTimestamp {
                frame: 0,
                host_time_ns: 0,
            },
        };
        let mut output = vec![f32::NAN; 1_400];
        processor.process(&[0.0; 700], &mut output, &info);
        output
    };
    // Prepared for 64 frames but handed 700: every sample is still written, and matches an engine
    // prepared large enough for the whole block.
    let sliced = render(64);
    assert!(
        sliced.iter().all(|s| s.is_finite()),
        "some output was left unwritten"
    );
    assert_eq!(sliced, render(MAX_BLOCK));
}
