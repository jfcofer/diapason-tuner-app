//! The half of the engine that runs on the audio thread.

use diapason_audio_io::{AudioCallback, CallbackInfo};
use diapason_dsp::level::RmsMeter;
use diapason_dsp::osc::SineOscillator;

use crate::{Command, EngineSnapshot};

/// The real-time half of the engine, created by [`crate::Engine::prepare`] and handed to a backend.
///
/// It owns everything the callback touches, allocated up front. Each callback drains pending
/// commands, meters the input, renders the output and publishes a snapshot. It allocates nothing,
/// takes no lock, makes no syscall and cannot panic (`AGENTS.md` §6). Debug builds trap any
/// allocation with `assert_no_alloc`; release builds compile that check out.
pub struct Processor {
    pub(crate) commands: rtrb::Consumer<Command>,
    pub(crate) snapshots: triple_buffer::Input<EngineSnapshot>,
    pub(crate) snapshot: EngineSnapshot,
    pub(crate) tone: SineOscillator,
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
        while let Ok(command) = self.commands.pop() {
            self.apply(command);
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

        self.snapshot.frames += info.frames as u64;
        self.snapshot.input_rms = self.meter.last();
        self.snapshots.write(self.snapshot);
    }

    fn apply(&mut self, command: Command) {
        match command {
            Command::StartTone {
                frequency_hz,
                amplitude,
            } => {
                self.tone.set_frequency(frequency_hz);
                self.tone.set_amplitude(amplitude);
                self.snapshot.tone_hz = Some(self.tone.frequency());
            }
            Command::StopTone => {
                self.tone.set_amplitude(0.0);
                self.snapshot.tone_hz = None;
            }
        }
        self.snapshot.commands_applied += 1;
    }
}
