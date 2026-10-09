//! The decisions: when to open, rebuild, give up, and what to replay.

use diapason_audio_io::{AudioBackend, AudioError, BackendReport, StreamConfig, StreamHandle};
use diapason_engine::{Command, Engine, EngineError, EngineSnapshot};

/// The largest block the session asks a backend for. Larger platform callbacks are cut to it.
/// 1024 frames is ~21 ms at 48 kHz, above any low-latency burst, and costs a few kilobytes.
pub const MAX_BLOCK_FRAMES: usize = 1024;

/// The rate the engine is prepared for. Backends that pick the device's native rate (Android)
/// ignore it, and the engine follows the rate granted (`docs/PLATFORM_AUDIO.md` §2).
const PREFERRED_SAMPLE_RATE: u32 = 48_000;
const OUTPUT_CHANNELS: usize = 2;
const INPUT_CHANNELS: usize = 1;

/// The first wait before reopening a broken stream. Each failure doubles it, up to the cap.
const FIRST_RETRY_NS: u64 = 50_000_000;
const MAX_RETRY_NS: u64 = 2_000_000_000;
/// How long recovery keeps trying before it reports the device as unavailable.
const GIVE_UP_NS: u64 = 10_000_000_000;

/// Where the session is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// No stream, and none wanted.
    #[default]
    Stopped,
    /// A stream is open and running.
    Running,
    /// The stream broke or would not open; the session is waiting to reopen it.
    Recovering,
    /// The session gave up. [`SessionSnapshot::fault`] says why. Starting again retries.
    Failed,
}

/// Why something failed, in the terms the UI offers a recovery for (`docs/ARCHITECTURE.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The microphone permission is missing.
    PermissionDenied,
    /// The device is gone, busy or broken. Worth retrying.
    DeviceUnavailable,
    /// The device refused the configuration. Retrying the same request will not help.
    ConfigurationUnsupported,
    /// A bug in this app, not in the device.
    Internal,
}

impl From<&AudioError> for Fault {
    fn from(error: &AudioError) -> Self {
        match error {
            AudioError::PermissionDenied => Self::PermissionDenied,
            AudioError::Disconnected | AudioError::Platform { .. } => Self::DeviceUnavailable,
            AudioError::UnsupportedConfiguration(_) => Self::ConfigurationUnsupported,
            AudioError::InvalidConfig(_)
            | AudioError::AlreadyOpen
            | AudioError::NotOpen
            | AudioError::BufferMismatch(_) => Self::Internal,
        }
    }
}

/// Everything the UI and the diagnostics overlay need, at one moment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionSnapshot {
    /// Where the session is in its lifecycle.
    pub state: SessionState,
    /// The backend's name.
    pub backend: &'static str,
    /// The engine's latest snapshot. The default while no stream is open.
    pub engine: EngineSnapshot,
    /// What the device granted. `None` while no stream is open.
    pub config: Option<StreamConfig>,
    /// Whether the open stream has the microphone.
    pub input_active: bool,
    /// Callbacks completed by the current stream.
    pub callbacks: u64,
    /// The current stream's slowest callback, in nanoseconds.
    pub worst_callback_ns: u64,
    /// Blocks the microphone had not delivered in time, on the current stream.
    pub input_underruns: u64,
    /// Path, preset, burst and xruns, as the backend reports them.
    pub report: BackendReport,
    /// Streams rebuilt after the platform broke one: a route change or a lost device. The UI shows
    /// a brief indicator when this rises (`docs/PLATFORM_AUDIO.md` §2).
    pub rebuilds: u32,
    /// Why the session is [`SessionState::Failed`].
    pub fault: Option<Fault>,
    /// Why the stream is running without the microphone it was asked for.
    pub input_fault: Option<Fault>,
    /// The most recent error from the backend, verbatim, for support reports.
    pub last_error: Option<AudioError>,
    /// Commands the audio thread could not take because its queue was full. They are not lost:
    /// desired state is replayed on the next rebuild.
    pub commands_dropped: u32,
}

/// What has been asked for. Replayed into every new engine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Desired {
    input: bool,
    tone: Option<(f32, f32)>,
}

/// The open stream's control side.
struct Live {
    engine: Engine,
    handle: StreamHandle,
    input: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct Retry {
    attempt: u32,
    next_at_ns: u64,
    since_ns: u64,
    after_disconnect: bool,
}

/// The session's decisions, with no thread and no clock of its own (`docs/adr/0022`).
///
/// Call [`tick`](Self::tick) regularly with a monotonic time. Each tick checks the open stream and
/// rebuilds it if it broke or no longer matches what was asked for.
pub struct Supervisor<B> {
    backend: B,
    desired: Desired,
    live: Option<Live>,
    state: SessionState,
    retry: Retry,
    rebuilds: u32,
    fault: Option<Fault>,
    input_fault: Option<Fault>,
    last_error: Option<AudioError>,
    commands_dropped: u32,
}

impl<B: AudioBackend> Supervisor<B> {
    /// A stopped session over `backend`, which must be closed.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            desired: Desired::default(),
            live: None,
            state: SessionState::Stopped,
            retry: Retry::default(),
            rebuilds: 0,
            fault: None,
            input_fault: None,
            last_error: None,
            commands_dropped: 0,
        }
    }

    /// The backend, for a test that has to pump `OfflineBackend` by hand.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Run the stream, with the microphone if `input`.
    ///
    /// Asking for the microphone again clears an earlier input fault, so this is also how the UI
    /// retries it, for example after the permission is granted. On a running session a change of
    /// `input` takes effect on the next [`tick`](Self::tick).
    pub fn start(&mut self, input: bool, now_ns: u64) {
        self.desired.input = input;
        self.input_fault = None;
        if matches!(self.state, SessionState::Stopped | SessionState::Failed) {
            self.fault = None;
            self.retry = Retry::default();
            self.open_or_retry(now_ns, false);
        }
    }

    /// Close the stream and forget about recovering it.
    pub fn stop(&mut self) {
        self.close_stream();
        self.state = SessionState::Stopped;
        self.retry = Retry::default();
        self.fault = None;
    }

    /// Play a test tone, now and after every rebuild.
    pub fn start_tone(&mut self, frequency_hz: f32, amplitude: f32) {
        self.desired.tone = Some((frequency_hz, amplitude));
        self.send(Command::StartTone {
            frequency_hz,
            amplitude,
        });
    }

    /// Stop the test tone.
    pub fn stop_tone(&mut self) {
        self.desired.tone = None;
        self.send(Command::StopTone);
    }

    /// Check the stream, and rebuild it if it broke, if a retry is due, or if it no longer
    /// matches what was asked for. `now_ns` is monotonic; its epoch does not matter.
    pub fn tick(&mut self, now_ns: u64) {
        match self.state {
            SessionState::Running => {
                let Some(live) = &self.live else {
                    return;
                };
                if live.handle.disconnected() {
                    self.close_stream();
                    self.state = SessionState::Recovering;
                    self.retry = Retry {
                        attempt: 0,
                        next_at_ns: now_ns.saturating_add(FIRST_RETRY_NS),
                        since_ns: now_ns,
                        after_disconnect: true,
                    };
                } else if live.input != self.wants_input() {
                    self.close_stream();
                    self.open_or_retry(now_ns, false);
                }
            }
            SessionState::Recovering if now_ns >= self.retry.next_at_ns => {
                let after_disconnect = self.retry.after_disconnect;
                self.open_or_retry(now_ns, after_disconnect);
            }
            SessionState::Recovering | SessionState::Stopped | SessionState::Failed => {}
        }
    }

    /// The session as it stands. Reads only counters and the engine's latest snapshot.
    pub fn snapshot(&mut self) -> SessionSnapshot {
        let (engine, handle, input_active) = match &mut self.live {
            Some(live) => (live.engine.snapshot(), Some(&live.handle), live.input),
            None => (EngineSnapshot::default(), None, false),
        };
        SessionSnapshot {
            state: self.state,
            backend: self.backend.name(),
            engine,
            config: self.backend.actual_config(),
            input_active,
            callbacks: handle.map_or(0, StreamHandle::callbacks),
            worst_callback_ns: handle.map_or(0, StreamHandle::worst_callback_ns),
            input_underruns: handle.map_or(0, StreamHandle::input_underruns),
            report: self.backend.report(),
            rebuilds: self.rebuilds,
            fault: self.fault,
            input_fault: self.input_fault,
            last_error: self.last_error.clone(),
            commands_dropped: self.commands_dropped,
        }
    }

    /// The microphone is wanted, and has not failed since it was last asked for.
    fn wants_input(&self) -> bool {
        self.desired.input && self.input_fault.is_none()
    }

    /// Open a stream; on failure, schedule the next attempt or give up.
    fn open_or_retry(&mut self, now_ns: u64, after_disconnect: bool) {
        let error = match self.open_stream() {
            Ok(()) => {
                if after_disconnect {
                    self.rebuilds = self.rebuilds.saturating_add(1);
                }
                self.state = SessionState::Running;
                self.retry = Retry::default();
                return;
            }
            Err(error) => error,
        };
        let fault = Fault::from(&error);
        self.last_error = Some(error);
        let since_ns = if self.state == SessionState::Recovering {
            self.retry.since_ns
        } else {
            now_ns
        };
        // Only a missing or busy device is worth waiting for. A refused configuration or a bug
        // fails the same way every time.
        if fault != Fault::DeviceUnavailable || now_ns.saturating_sub(since_ns) >= GIVE_UP_NS {
            self.state = SessionState::Failed;
            self.fault = Some(fault);
            return;
        }
        let attempt = if self.state == SessionState::Recovering {
            self.retry.attempt.saturating_add(1)
        } else {
            0
        };
        let delay = FIRST_RETRY_NS
            .saturating_mul(1 << attempt.min(16))
            .min(MAX_RETRY_NS);
        self.state = SessionState::Recovering;
        self.retry = Retry {
            attempt,
            next_at_ns: now_ns.saturating_add(delay),
            since_ns,
            after_disconnect,
        };
    }

    /// Open with the microphone if it is wanted, falling back to output only if the microphone
    /// fails, so the metronome never depends on it (`docs/PLATFORM_AUDIO.md` §2).
    fn open_stream(&mut self) -> Result<(), AudioError> {
        if self.wants_input() {
            match self.try_open(true) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    self.input_fault = Some(Fault::from(&error));
                    self.last_error = Some(error);
                }
            }
        }
        self.try_open(false)
    }

    /// Build a fresh engine, replay desired state into it, and hand its processor to the backend.
    fn try_open(&mut self, input: bool) -> Result<(), AudioError> {
        let (mut engine, processor) =
            Engine::prepare(MAX_BLOCK_FRAMES, PREFERRED_SAMPLE_RATE).map_err(engine_error)?;
        if let Some((frequency_hz, amplitude)) = self.desired.tone {
            engine
                .send(Command::StartTone {
                    frequency_hz,
                    amplitude,
                })
                .map_err(engine_error)?;
        }
        let config = StreamConfig {
            sample_rate: PREFERRED_SAMPLE_RATE,
            max_block_frames: MAX_BLOCK_FRAMES,
            input_channels: if input { INPUT_CHANNELS } else { 0 },
            output_channels: OUTPUT_CHANNELS,
        };
        let handle = self.backend.open(config, Box::new(processor))?;
        self.live = Some(Live {
            engine,
            handle,
            input,
        });
        Ok(())
    }

    fn close_stream(&mut self) {
        self.live = None;
        // Closing is idempotent, and the stream is closed whatever it returns.
        if let Err(error) = self.backend.close() {
            self.last_error = Some(error);
        }
    }

    fn send(&mut self, command: Command) {
        if let Some(live) = &mut self.live
            && live.engine.send(command).is_err()
        {
            self.commands_dropped = self.commands_dropped.saturating_add(1);
        }
    }
}

/// An engine that refuses the session's own constants is a bug here, not a device problem.
fn engine_error(error: EngineError) -> AudioError {
    match error {
        EngineError::InvalidConfig(field) => AudioError::InvalidConfig(field),
        EngineError::QueueFull => AudioError::InvalidConfig("a fresh command queue was full"),
    }
}

#[cfg(test)]
mod tests;
