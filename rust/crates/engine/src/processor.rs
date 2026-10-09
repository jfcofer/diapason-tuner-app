//! The half of the engine that runs on the audio thread.

use diapason_audio_io::{AudioCallback, CallbackInfo};
use diapason_dsp::level::RmsMeter;
use diapason_dsp::osc::SineOscillator;

use crate::{COMMAND_CAPACITY, Command, EngineSnapshot, meter_window};

/// The real-time half of the engine, created by [`crate::Engine::prepare`] and handed to a backend.
///
/// It owns everything the callback touches, allocated up front. Each callback drains pending
/// commands, follows the stream's actual sample rate, meters the input, renders the output and
/// publishes a snapshot. It allocates nothing, takes no lock, makes no syscall and cannot panic
/// (`AGENTS.md` §6).
///
/// In debug builds the body runs inside `assert_no_alloc`, which traps an allocation wherever its
/// allocator is installed as the global one: today the zero-allocation test, from `T-002b` the
/// debug app. Release builds compile the check out.
pub struct Processor {
    pub(crate) commands: rtrb::Consumer<Command>,
    pub(crate) snapshots: triple_buffer::Input<EngineSnapshot>,
    pub(crate) snapshot: EngineSnapshot,
    pub(crate) tone: SineOscillator,
    /// The tone last asked for, in hertz and amplitude, so a rate change can rebuild it.
    pub(crate) tone_request: Option<(f32, f32)>,
    pub(crate) meter: RmsMeter,
    /// One channel of output, `max_block_size` long, rendered once and copied to every channel.
    pub(crate) mono: Box<[f32]>,
}

impl AudioCallback for Processor {
    fn process(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo) {
        assert_no_alloc::assert_no_alloc(|| self.render(input, output, info));
    }
}

impl Processor {
    fn render(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo) {
        // Bounded, so a producer refilling the queue mid-drain cannot hold the callback.
        for _ in 0..COMMAND_CAPACITY {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            self.apply(command);
        }

        if info.sample_rate != self.snapshot.sample_rate {
            self.follow_sample_rate(info.sample_rate);
        }

        if info.input_channels > 0 {
            for &sample in input.iter().step_by(info.input_channels) {
                self.meter.process_sample(sample);
            }
        }

        // A backend may hand over more than it promised; render in slices of what was prepared
        // rather than index past the scratch buffer.
        let channels = info.output_channels;
        if channels > 0 {
            for slice in output.chunks_mut(self.mono.len() * channels) {
                let Some(mono) = self.mono.get_mut(..slice.len() / channels) else {
                    break;
                };
                self.tone.fill(mono);
                for (frame, &sample) in slice.chunks_exact_mut(channels).zip(mono.iter()) {
                    frame.fill(sample);
                }
            }
        }

        self.snapshot.clock = info.timestamp;
        self.snapshot.frames = info.timestamp.frame + info.frames as u64;
        self.snapshot.input_rms = self.meter.last();
        self.snapshots.write(self.snapshot);
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::StartTone {
                frequency_hz,
                amplitude,
            } => {
                self.tone_request = Some((frequency_hz, amplitude));
                self.tone.set_frequency(frequency_hz);
                self.tone.set_amplitude(amplitude);
                self.snapshot.tone_hz = Some(self.tone.frequency());
            }
            Command::StopTone => {
                self.tone_request = None;
                self.tone.set_amplitude(0.0);
                self.snapshot.tone_hz = None;
            }
        }
        self.snapshot.commands_applied += 1;
    }

    /// The device runs at a different rate from the one the engine was prepared for
    /// (`docs/PLATFORM_AUDIO.md` §2). Rebuild everything rate-dependent, so a 440 Hz tone stays
    /// 440 Hz and a meter window stays 50 ms. Plain values, no allocation.
    fn follow_sample_rate(&mut self, sample_rate: u32) {
        self.snapshot.sample_rate = sample_rate;
        self.meter = RmsMeter::new(meter_window(sample_rate));
        self.tone = SineOscillator::new(sample_rate);
        if let Some((frequency_hz, amplitude)) = self.tone_request {
            self.tone.set_frequency(frequency_hz);
            self.tone.set_amplitude(amplitude);
            self.snapshot.tone_hz = Some(self.tone.frequency());
        }
    }
}
