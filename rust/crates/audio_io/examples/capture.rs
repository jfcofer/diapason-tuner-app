//! Records the microphone through `AAudioBackend` to a WAV on the device (`T-026`).
//!
//! Development only: an example links into no app, so it cannot reach the prod flavour. Run it with
//! `just capture`. The stream is the tuner's: duplex, 48 kHz, `VoiceRecognition`, silent output.
//!
//! The file is 24-bit integer PCM: `dsp/tests/recorded.rs` and `cargo xtask fixtures` read integer PCM
//! only (`docs/adr/0030`), and 24 bits keep anything the Redmi delivers, which looks like 16.
//!
//! It fails rather than keep a take that would mislead: a long run of exact zeros, or audio the stream
//! reports lost after warm-up. Start-up draining is the backend's normal behaviour (`docs/adr/0025`),
//! so the first second is discarded and not counted. Two limits, from `T-026`: whether the OS's
//! microphone toggle produces zeros on the Redmi is unproven, and AAudio lets an input device report
//! no xruns at all, so a zero there excludes nothing.

#[cfg(target_os = "android")]
mod device {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use diapason_audio_io::{
        AAudioBackend, AudioBackend, AudioCallback, BackendReport, CallbackInfo, InputPreset,
        StreamConfig, StreamHandle,
    };

    const RATE: u32 = 48_000;
    const WARM_UP: Duration = Duration::from_secs(1);
    /// Five seconds of microphone: the writer only moves samples into memory, so this is generous.
    const RING_FRAMES: usize = 5 * RATE as usize;
    /// 2²³: an `f32` in [−1, 1) to a 24-bit sample.
    const FULL_SCALE: f64 = 8_388_608.0;
    /// At or above this is clipped. A 16-bit source's positive rail is 0.99997, never 1.0.
    const CLIP: f32 = 0.999;
    /// A run of exact zeros this long is not a quiet room: 100 ms. Real takes hold about 1 % exact
    /// zeros, scattered, so a share of zeros would not tell them from a muted stretch.
    const MUTED_RUN: usize = 4_800;
    /// With no new frames for this long, the stream has stopped for good (`android.rs`).
    const STALLED: Duration = Duration::from_secs(2);

    /// Copies the microphone into the ring and nothing else. `rtrb` neither locks nor allocates
    /// (`AGENTS.md` §6). A full ring counts the loss, which fails the take afterwards.
    struct Tap {
        ring: rtrb::Producer<f32>,
        lost: Arc<AtomicU64>,
    }

    impl AudioCallback for Tap {
        fn process(&mut self, input: &[f32], output: &mut [f32], _: &CallbackInfo) {
            output.fill(0.0);
            for &sample in input {
                if self.ring.push(sample).is_err() {
                    self.lost.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    /// What the stream counts against the microphone: any rise during a take loses audio. `shed`
    /// should not move after warm-up (`docs/adr/0025`), and `input_xruns` may be fixed at zero.
    #[derive(Clone, Copy)]
    struct Losses {
        short_blocks: u64,
        shed: u64,
        input_xruns: i64,
    }

    impl Losses {
        fn read(handle: &StreamHandle, report: &BackendReport) -> Self {
            let all = i64::from(report.xruns.unwrap_or(0));
            let output = i64::from(report.output_xruns.unwrap_or(0));
            Self {
                short_blocks: handle.input_underruns(),
                shed: handle.input_frames_shed(),
                input_xruns: all - output,
            }
        }
    }

    /// Move `wanted` samples into `take`, or say why they will never arrive: an output failure stops
    /// the callbacks for good, without a disconnect the input could report.
    fn drain(
        ring: &mut rtrb::Consumer<f32>,
        handle: &StreamHandle,
        take: &mut Vec<f32>,
        wanted: usize,
    ) -> Result<(), String> {
        let mut frames = handle.frames();
        let mut moved = Instant::now();
        while take.len() < wanted {
            if let Ok(sample) = ring.pop() {
                take.push(sample);
                continue;
            }
            if handle.disconnected() {
                return Err("the stream disconnected during the take".into());
            }
            if handle.frames() != frames {
                frames = handle.frames();
                moved = Instant::now();
            } else if moved.elapsed() > STALLED {
                return Err("the stream stopped delivering audio".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    /// The longest run of samples that are exactly zero.
    fn longest_zero_run(take: &[f32]) -> usize {
        let (mut longest, mut run) = (0, 0);
        for &sample in take {
            run = if sample == 0.0 { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        longest
    }

    fn write_wav(path: &str, take: &[f32], rate: u32) -> Result<(), hound::Error> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;
        for &sample in take {
            let scaled = (f64::from(sample) * FULL_SCALE).round();
            writer.write_sample(scaled.clamp(-FULL_SCALE, FULL_SCALE - 1.0) as i32)?;
        }
        writer.finalize()
    }

    fn decibels(level: f64) -> f64 {
        if level > 0.0 {
            20.0 * level.log10()
        } else {
            f64::NEG_INFINITY
        }
    }

    pub fn run(path: &str, seconds: u32) -> Result<(), String> {
        let config = StreamConfig {
            sample_rate: RATE,
            max_block_frames: 1024,
            input_channels: 1,
            output_channels: 2,
        };
        let (producer, mut consumer) = rtrb::RingBuffer::new(RING_FRAMES);
        let lost = Arc::new(AtomicU64::new(0));
        let tap = Tap {
            ring: producer,
            lost: Arc::clone(&lost),
        };
        let mut backend = AAudioBackend::new(InputPreset::VoiceRecognition);
        let handle = backend
            .open(config, Box::new(tap))
            .map_err(|e| format!("could not open the stream: {e}"))?;
        let granted = backend
            .actual_config()
            .ok_or("the stream closed itself after opening")?;

        // Warm up, then take the baselines before draining what arrived, so that a glitch while
        // draining counts against the take.
        thread::sleep(WARM_UP);
        let before = Losses::read(&handle, &backend.report());
        let lost_before = lost.load(Ordering::Relaxed);
        while consumer.pop().is_ok() {}

        let wanted = seconds as usize * granted.sample_rate as usize;
        let mut take = Vec::with_capacity(wanted);
        println!("● recording {seconds} s");
        let drained = drain(&mut consumer, &handle, &mut take, wanted);
        println!("■ stopped");

        let report = backend.report();
        let after = Losses::read(&handle, &report);
        let lost_frames = lost.load(Ordering::Relaxed) - lost_before;
        let disconnected = handle.disconnected();
        backend
            .close()
            .map_err(|e| format!("could not close the stream: {e}"))?;
        drained?;

        let peak = take.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
        let energy: f64 = take.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
        let rms = (energy / take.len() as f64).sqrt();
        let clipped = take.iter().filter(|s| s.abs() >= CLIP).count();
        let zero_run = longest_zero_run(&take);
        let output_xruns = report.output_xruns.unwrap_or(0);
        let short_blocks = after.short_blocks - before.short_blocks;
        let shed = after.shed - before.shed;
        let xruns = after.input_xruns - before.input_xruns;

        println!(
            "stream:  {} Hz, burst {:?}, preset {:?} (requested {:?})",
            granted.sample_rate,
            report.frames_per_burst,
            report.input_preset,
            report.requested_input_preset
        );
        println!(
            "level:   peak {:.1} dBFS, RMS {:.1} dBFS, {clipped} clipped",
            decibels(f64::from(peak)),
            decibels(rms)
        );
        println!(
            "losses:  {short_blocks} short blocks, {shed} frames shed, {xruns} input xruns, {lost_frames} frames lost, {output_xruns} output xruns"
        );
        println!("zeros:   longest run {zero_run} samples");

        if zero_run >= MUTED_RUN {
            return Err(format!(
                "{zero_run} samples in a row are exactly zero: the microphone was muted"
            ));
        }
        if disconnected {
            return Err("the stream disconnected during the take".into());
        }
        if short_blocks > 0 || shed > 0 || xruns > 0 || lost_frames > 0 {
            return Err("the take lost audio".into());
        }
        write_wav(path, &take, granted.sample_rate)
            .map_err(|e| format!("could not write {path}: {e}"))?;
        println!("clipping is reported, not fatal; wrote {path}");
        Ok(())
    }

    pub fn main() -> ExitCode {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let parsed = match args.as_slice() {
            [path, seconds] => seconds.parse().ok().map(|s: u32| (path, s)),
            _ => None,
        };
        let Some((path, seconds)) = parsed.filter(|(_, s)| *s > 0) else {
            eprintln!("usage: capture <device-path.wav> <seconds>");
            return ExitCode::FAILURE;
        };
        match run(path, seconds) {
            Ok(()) => ExitCode::SUCCESS,
            Err(reason) => {
                eprintln!("✗ {reason}");
                ExitCode::FAILURE
            }
        }
    }
}

#[cfg(target_os = "android")]
fn main() -> std::process::ExitCode {
    device::main()
}

#[cfg(not(target_os = "android"))]
fn main() -> std::process::ExitCode {
    eprintln!("capture runs on an Android device: use `just capture`");
    std::process::ExitCode::FAILURE
}
