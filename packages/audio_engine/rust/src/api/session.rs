//! The audio session across the bridge: commands in, ~30 Hz snapshots out.
//!
//! Type mapping only (`AGENTS.md` §5). Every decision is in `diapason_session`, which `cargo test`
//! reaches on virtual time. Enums here carry no data, so the generated Dart needs no code
//! generator beyond flutter_rust_bridge (`docs/ARCHITECTURE.md` §8).

use diapason_session::{
    Fault, InputPreset, Session, SessionError, SessionSnapshot, SessionState,
    spawn_platform_session,
};

use crate::frb_generated::StreamSink;

/// The app's one audio session. Dropping it on the Dart side stops the stream.
#[flutter_rust_bridge::frb(opaque)]
pub struct AudioSession {
    session: Session,
}

impl AudioSession {
    /// Start the session thread on this platform's backend. No stream opens until [`start`].
    ///
    /// [`start`]: AudioSession::start
    ///
    /// # Errors
    /// [`AudioSessionError::Spawn`] if the thread cannot be created.
    #[flutter_rust_bridge::frb(sync)]
    pub fn spawn() -> Result<AudioSession, AudioSessionError> {
        Ok(Self {
            session: spawn_platform_session()?,
        })
    }

    /// Run the stream, with the microphone if `input`. Returns at once.
    ///
    /// # Errors
    /// [`AudioSessionError::Gone`] if the session has shut down.
    #[flutter_rust_bridge::frb(sync)]
    pub fn start(&self, input: bool) -> Result<(), AudioSessionError> {
        Ok(self.session.start(input)?)
    }

    /// Close the stream. Returns at once.
    ///
    /// # Errors
    /// [`AudioSessionError::Gone`] if the session has shut down.
    #[flutter_rust_bridge::frb(sync)]
    pub fn stop(&self) -> Result<(), AudioSessionError> {
        Ok(self.session.stop()?)
    }

    /// Play a test tone, kept across stream rebuilds until stopped.
    ///
    /// # Errors
    /// [`AudioSessionError::Gone`] if the session has shut down.
    #[flutter_rust_bridge::frb(sync)]
    pub fn start_tone(&self, frequency_hz: f32, amplitude: f32) -> Result<(), AudioSessionError> {
        Ok(self.session.start_tone(frequency_hz, amplitude)?)
    }

    /// Stop the test tone.
    ///
    /// # Errors
    /// [`AudioSessionError::Gone`] if the session has shut down.
    #[flutter_rust_bridge::frb(sync)]
    pub fn stop_tone(&self) -> Result<(), AudioSessionError> {
        Ok(self.session.stop_tone()?)
    }

    /// Deliver a snapshot to `sink` about 30 times a second, replacing any earlier subscriber.
    /// Delivery stops when Dart cancels the stream.
    ///
    /// # Errors
    /// [`AudioSessionError::Gone`] if the session has shut down.
    pub fn snapshots(&self, sink: StreamSink<SessionSnapshotDto>) -> Result<(), AudioSessionError> {
        Ok(self
            .session
            .subscribe(move |snapshot| sink.add(snapshot.into()).is_ok())?)
    }
}

/// Why the session could not take a request.
#[derive(Debug)]
pub enum AudioSessionError {
    /// The session thread could not be started.
    Spawn,
    /// The session has shut down.
    Gone,
}

impl From<SessionError> for AudioSessionError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::Spawn => Self::Spawn,
            SessionError::Gone => Self::Gone,
        }
    }
}

/// Where the session is in its lifecycle. Mirrors `diapason_session::SessionState`.
pub enum SessionStateDto {
    /// No stream, and none wanted.
    Stopped,
    /// A stream is open and running.
    Running,
    /// Waiting to reopen a broken stream.
    Recovering,
    /// Gave up; see the fault.
    Failed,
}

/// Why something failed. Mirrors `diapason_session::Fault`.
pub enum FaultDto {
    /// The microphone permission is missing.
    PermissionDenied,
    /// The device is gone, busy or broken.
    DeviceUnavailable,
    /// The device refused the configuration.
    ConfigurationUnsupported,
    /// A bug in this app.
    Internal,
}

/// The microphone processing the device applied. Mirrors `diapason_audio_io::InputPreset`.
pub enum InputPresetDto {
    /// No processing.
    Unprocessed,
    /// Automatic gain off on most devices.
    VoiceRecognition,
    /// Anything else; `input_preset_code` has the platform's value.
    Other,
}

/// One session snapshot, flattened for Dart. Field meanings are documented on
/// `diapason_session::SessionSnapshot`, which this mirrors.
pub struct SessionSnapshotDto {
    /// Lifecycle state.
    pub state: SessionStateDto,
    /// Backend name.
    pub backend: String,
    /// The rate the stream runs at, or the rate the engine was prepared for.
    pub sample_rate: u32,
    /// Largest block the callback may be given. `None` with no stream open.
    pub max_block_frames: Option<u32>,
    /// Whether the open stream has the microphone.
    pub input_active: bool,
    /// Input RMS over the last 50 ms window.
    pub input_rms: f32,
    /// The test tone's frequency while it plays.
    pub tone_hz: Option<f32>,
    /// Frames the current stream has played.
    pub frames: u64,
    /// Callbacks the current stream has completed.
    pub callbacks: u64,
    /// Slowest callback of the current stream, in nanoseconds.
    pub worst_callback_ns: u64,
    /// Blocks the microphone delivered late.
    pub input_underruns: u64,
    /// Platform-counted underruns and overruns.
    pub xruns: Option<u32>,
    /// The output's burst size, in frames.
    pub frames_per_burst: Option<u32>,
    /// The input preset obtained.
    pub input_preset: Option<InputPresetDto>,
    /// The platform's raw value for the input preset obtained.
    pub input_preset_code: Option<i32>,
    /// Whether the output got the low-latency path.
    pub output_low_latency: Option<bool>,
    /// Whether the output got exclusive mode.
    pub output_exclusive: Option<bool>,
    /// Whether the input got the low-latency path.
    pub input_low_latency: Option<bool>,
    /// Whether the input got exclusive mode.
    pub input_exclusive: Option<bool>,
    /// Streams rebuilt after the platform broke one.
    pub rebuilds: u32,
    /// Why the session failed.
    pub fault: Option<FaultDto>,
    /// Why the stream runs without the microphone it was asked for.
    pub input_fault: Option<FaultDto>,
    /// The most recent backend error, as text, for support reports.
    pub last_error: Option<String>,
    /// Commands the audio thread's queue could not take.
    pub commands_dropped: u32,
}

impl From<&SessionSnapshot> for SessionSnapshotDto {
    fn from(snapshot: &SessionSnapshot) -> Self {
        let report = snapshot.report;
        Self {
            state: snapshot.state.into(),
            backend: snapshot.backend.to_owned(),
            sample_rate: snapshot.engine.sample_rate,
            max_block_frames: snapshot
                .config
                .and_then(|config| u32::try_from(config.max_block_frames).ok()),
            input_active: snapshot.input_active,
            input_rms: snapshot.engine.input_rms,
            tone_hz: snapshot.engine.tone_hz,
            frames: snapshot.engine.frames,
            callbacks: snapshot.callbacks,
            worst_callback_ns: snapshot.worst_callback_ns,
            input_underruns: snapshot.input_underruns,
            xruns: report.xruns,
            frames_per_burst: report.frames_per_burst,
            input_preset: report.input_preset.map(Into::into),
            input_preset_code: report.input_preset.and_then(|preset| match preset {
                InputPreset::Other(code) => Some(code),
                InputPreset::Unprocessed | InputPreset::VoiceRecognition => None,
            }),
            output_low_latency: report.output_path.map(|path| path.low_latency),
            output_exclusive: report.output_path.map(|path| path.exclusive),
            input_low_latency: report.input_path.map(|path| path.low_latency),
            input_exclusive: report.input_path.map(|path| path.exclusive),
            rebuilds: snapshot.rebuilds,
            fault: snapshot.fault.map(Into::into),
            input_fault: snapshot.input_fault.map(Into::into),
            last_error: snapshot.last_error.as_ref().map(ToString::to_string),
            commands_dropped: snapshot.commands_dropped,
        }
    }
}

impl From<SessionState> for SessionStateDto {
    fn from(state: SessionState) -> Self {
        match state {
            SessionState::Stopped => Self::Stopped,
            SessionState::Running => Self::Running,
            SessionState::Recovering => Self::Recovering,
            SessionState::Failed => Self::Failed,
        }
    }
}

impl From<Fault> for FaultDto {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::PermissionDenied => Self::PermissionDenied,
            Fault::DeviceUnavailable => Self::DeviceUnavailable,
            Fault::ConfigurationUnsupported => Self::ConfigurationUnsupported,
            Fault::Internal => Self::Internal,
        }
    }
}

impl From<InputPreset> for InputPresetDto {
    fn from(preset: InputPreset) -> Self {
        match preset {
            InputPreset::Unprocessed => Self::Unprocessed,
            InputPreset::VoiceRecognition => Self::VoiceRecognition,
            InputPreset::Other(_) => Self::Other,
        }
    }
}
