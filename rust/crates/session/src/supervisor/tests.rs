//! The supervisor on virtual time, through `OfflineBackend`: no thread, no wall clock.

use diapason_audio_io::{
    AudioBackend, AudioCallback, AudioError, BlockPattern, OfflineBackend, StreamConfig,
    StreamHandle, StreamTimestamp,
};

use super::*;

const MS: u64 = 1_000_000;

/// `OfflineBackend`, plus opens that fail on demand and a count of open attempts.
struct Flaky {
    inner: OfflineBackend,
    /// Every open fails with this.
    fail_all: Option<AudioError>,
    /// Opens that ask for the microphone fail with this.
    fail_input: Option<AudioError>,
    /// Open attempts so far, failed or not.
    opens: u32,
    /// Open attempts that asked for the microphone.
    input_opens: u32,
}

impl Flaky {
    fn new() -> Self {
        Self {
            inner: OfflineBackend::new(BlockPattern::Fixed(256)),
            fail_all: None,
            fail_input: None,
            opens: 0,
            input_opens: 0,
        }
    }
}

impl AudioBackend for Flaky {
    fn name(&self) -> &'static str {
        "flaky"
    }

    fn open(
        &mut self,
        config: StreamConfig,
        callback: Box<dyn AudioCallback>,
    ) -> diapason_audio_io::Result<StreamHandle> {
        self.opens += 1;
        if config.input_channels > 0 {
            self.input_opens += 1;
        }
        if let Some(error) = &self.fail_all {
            return Err(error.clone());
        }
        if config.input_channels > 0
            && let Some(error) = &self.fail_input
        {
            return Err(error.clone());
        }
        self.inner.open(config, callback)
    }

    fn actual_config(&self) -> Option<StreamConfig> {
        self.inner.actual_config()
    }

    fn timestamp(&self) -> Option<StreamTimestamp> {
        self.inner.timestamp()
    }

    fn close(&mut self) -> diapason_audio_io::Result<()> {
        self.inner.close()
    }
}

/// Push `frames` through whatever stream is open, so the engine applies its commands.
fn render(supervisor: &mut Supervisor<Flaky>, frames: usize) {
    let config = supervisor
        .backend_mut()
        .inner
        .actual_config()
        .expect("a stream is open");
    let input = vec![0.0; frames * config.input_channels];
    let mut output = vec![0.0; frames * config.output_channels];
    supervisor
        .backend_mut()
        .inner
        .render(&input, &mut output)
        .expect("render");
}

fn platform_error() -> AudioError {
    AudioError::Platform {
        operation: "open the output stream",
        code: -899,
    }
}

#[test]
fn a_stopped_session_reports_nothing_open() {
    let mut supervisor = Supervisor::new(Flaky::new());
    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Stopped);
    assert_eq!(snapshot.config, None);
    assert!(!snapshot.input_active);
    assert_eq!(snapshot.backend, "flaky");
}

#[test]
fn start_opens_a_duplex_stream_and_plays_the_tone_asked_for_before_it() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start_tone(440.0, 0.5);
    supervisor.start(true, 0);
    render(&mut supervisor, 1024);

    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running);
    assert!(snapshot.input_active);
    assert_eq!(snapshot.config.map(|c| c.input_channels), Some(1));
    assert_eq!(snapshot.engine.tone_hz, Some(440.0));
    assert!(snapshot.callbacks > 0);
}

#[test]
fn a_disconnect_rebuilds_after_the_first_backoff_and_replays_the_tone() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(true, 0);
    supervisor.start_tone(220.0, 0.25);
    render(&mut supervisor, 512);
    supervisor.backend_mut().inner.simulate_disconnect();

    supervisor.tick(100 * MS);
    assert_eq!(supervisor.snapshot().state, SessionState::Recovering);
    assert_eq!(
        supervisor.snapshot().config,
        None,
        "the broken stream is closed"
    );

    supervisor.tick(149 * MS);
    assert_eq!(supervisor.snapshot().state, SessionState::Recovering);
    supervisor.tick(150 * MS);
    render(&mut supervisor, 512);

    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running);
    assert_eq!(snapshot.rebuilds, 1);
    assert!(snapshot.input_active);
    assert_eq!(
        snapshot.engine.tone_hz,
        Some(220.0),
        "desired state was replayed"
    );
}

#[test]
fn recovery_backs_off_exponentially_and_gives_up_after_ten_seconds() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(false, 0);
    supervisor.backend_mut().fail_all = Some(platform_error());
    supervisor.backend_mut().inner.simulate_disconnect();
    supervisor.tick(0);

    // Step virtual time 1 ms at a time and note when each reopen is attempted.
    let mut attempts = Vec::new();
    let mut now = 0;
    while supervisor.snapshot().state == SessionState::Recovering {
        now += MS;
        let before = supervisor.backend_mut().opens;
        supervisor.tick(now);
        if supervisor.backend_mut().opens > before {
            attempts.push(now / MS);
        }
        assert!(now < 30_000 * MS, "never gave up");
    }

    assert_eq!(&attempts[..6], &[50, 150, 350, 750, 1550, 3150]);
    let gaps: Vec<u64> = attempts.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        gaps.iter().all(|&gap| gap <= 2000),
        "capped at 2 s: {gaps:?}"
    );
    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Failed);
    assert_eq!(snapshot.fault, Some(Fault::DeviceUnavailable));
    assert!(now >= 10_000 * MS, "gave up early, at {now}");
    assert_eq!(snapshot.rebuilds, 0);
}

#[test]
fn a_failed_microphone_falls_back_to_output_only_without_a_rebuild_loop() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.backend_mut().fail_input = Some(AudioError::PermissionDenied);
    supervisor.start(true, 0);

    let snapshot = supervisor.snapshot();
    assert_eq!(
        snapshot.state,
        SessionState::Running,
        "the output still runs"
    );
    assert!(!snapshot.input_active);
    assert_eq!(snapshot.input_fault, Some(Fault::PermissionDenied));

    let opens = supervisor.backend_mut().opens;
    for tick in 1..100 {
        supervisor.tick(tick * 33 * MS);
    }
    assert_eq!(
        supervisor.backend_mut().opens,
        opens,
        "no retry until asked"
    );

    // Asking again, say once the permission is granted, retries the microphone.
    supervisor.backend_mut().fail_input = None;
    supervisor.start(true, 4000 * MS);
    supervisor.tick(4033 * MS);
    let snapshot = supervisor.snapshot();
    assert!(snapshot.input_active);
    assert_eq!(snapshot.input_fault, None);
}

#[test]
fn changing_whether_the_microphone_is_wanted_rebuilds_without_counting_a_route_change() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(false, 0);
    assert!(!supervisor.snapshot().input_active);

    supervisor.start(true, 10 * MS);
    supervisor.tick(20 * MS);
    assert!(supervisor.snapshot().input_active);

    supervisor.start(false, 30 * MS);
    supervisor.tick(40 * MS);
    let snapshot = supervisor.snapshot();
    assert!(!snapshot.input_active, "the microphone is released");
    assert_eq!(snapshot.rebuilds, 0);
}

#[test]
fn a_refused_configuration_fails_at_once_and_start_retries_it() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.backend_mut().fail_all =
        Some(AudioError::UnsupportedConfiguration("no stereo".to_owned()));
    supervisor.start(false, 0);
    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Failed);
    assert_eq!(snapshot.fault, Some(Fault::ConfigurationUnsupported));
    assert!(snapshot.last_error.is_some());

    supervisor.backend_mut().fail_all = None;
    supervisor.start(false, MS);
    assert_eq!(supervisor.snapshot().state, SessionState::Running);
    assert_eq!(supervisor.snapshot().fault, None);
}

#[test]
fn stop_closes_the_stream_and_ends_recovery() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(true, 0);
    supervisor.backend_mut().inner.simulate_disconnect();
    supervisor.tick(MS);
    supervisor.stop();

    let opens = supervisor.backend_mut().opens;
    supervisor.tick(10_000 * MS);
    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Stopped);
    assert_eq!(snapshot.config, None);
    assert_eq!(supervisor.backend_mut().opens, opens);
}

#[test]
fn a_stopped_tone_stays_stopped_across_a_rebuild() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(false, 0);
    supervisor.start_tone(440.0, 0.5);
    supervisor.stop_tone();
    supervisor.backend_mut().inner.simulate_disconnect();
    supervisor.tick(0);
    supervisor.tick(FIRST_RETRY_NS);
    render(&mut supervisor, 512);
    assert_eq!(supervisor.snapshot().engine.tone_hz, None);
}

#[test]
fn a_device_lost_for_a_moment_keeps_the_microphone_wanted() {
    // Found in review: both opens failing used to be blamed on the microphone, so recovery came
    // back output-only and the tuner stayed deaf until restart.
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(true, 0);
    supervisor.backend_mut().inner.simulate_disconnect();
    supervisor.tick(0);

    supervisor.backend_mut().fail_all = Some(platform_error());
    supervisor.tick(FIRST_RETRY_NS);
    assert_eq!(supervisor.snapshot().state, SessionState::Recovering);
    assert_eq!(
        supervisor.snapshot().input_fault,
        None,
        "the device failed, not the microphone"
    );

    supervisor.backend_mut().fail_all = None;
    supervisor.tick(FIRST_RETRY_NS + 100 * MS);
    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running);
    assert!(
        snapshot.input_active,
        "the microphone came back with the device"
    );
    assert_eq!(snapshot.rebuilds, 1);
}

#[test]
fn a_command_dropped_on_a_full_queue_is_restored_on_the_next_tick() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(false, 0);
    // No callbacks run, so nothing drains the queue: fill it, then lose the StopTone.
    while supervisor.snapshot().commands_dropped == 0 {
        supervisor.start_tone(440.0, 0.5);
    }
    let dropped = supervisor.snapshot().commands_dropped;
    supervisor.stop_tone();
    assert_eq!(
        supervisor.snapshot().commands_dropped,
        dropped + 1,
        "the StopTone was dropped"
    );

    render(&mut supervisor, 256); // the audio thread drains the queue: the tone is on
    supervisor.tick(33 * MS); // the supervisor sends the desired state again
    render(&mut supervisor, 256);
    assert_eq!(
        supervisor.snapshot().engine.tone_hz,
        None,
        "the lost StopTone was restored"
    );
}

#[test]
fn stopping_clears_a_microphone_fault() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.backend_mut().fail_input = Some(AudioError::PermissionDenied);
    supervisor.start(true, 0);
    assert!(supervisor.snapshot().input_fault.is_some());
    supervisor.stop();
    assert_eq!(supervisor.snapshot().input_fault, None);
}

/// What `AAudio` returns for an input opened without `RECORD_AUDIO`: generic, so it says nothing
/// about the permission (`T-002b` part 2a, on the Redmi).
fn refused_input() -> AudioError {
    AudioError::Platform {
        operation: "open the input stream",
        code: -896,
    }
}

#[test]
fn a_denied_microphone_is_never_asked_for_and_is_reported_as_denied() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Denied);
    supervisor.start(true, 0);

    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running, "the output runs");
    assert!(!snapshot.input_active);
    assert_eq!(snapshot.input_fault, Some(Fault::PermissionDenied));
    assert_eq!(
        supervisor.backend_mut().input_opens,
        0,
        "the platform was never asked"
    );

    for tick in 1..100 {
        supervisor.tick(tick * 33 * MS);
    }
    assert_eq!(
        supervisor.backend_mut().input_opens,
        0,
        "and is not asked on a tick"
    );
}

#[test]
fn granting_the_microphone_while_running_brings_it_back_and_replays_the_tone() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Denied);
    supervisor.start(true, 0);
    supervisor.start_tone(440.0, 0.5);

    supervisor.set_microphone_access(MicrophoneAccess::Granted);
    assert_eq!(supervisor.snapshot().input_fault, None, "cleared at once");
    supervisor.tick(33 * MS);
    render(&mut supervisor, 512);

    let snapshot = supervisor.snapshot();
    assert!(snapshot.input_active);
    assert_eq!(
        snapshot.engine.tone_hz,
        Some(440.0),
        "desired state was replayed"
    );
    assert_eq!(snapshot.rebuilds, 0, "not a route change");
}

#[test]
fn denying_the_microphone_while_running_releases_it_and_keeps_the_output() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Granted);
    supervisor.start(true, 0);
    supervisor.start_tone(220.0, 0.25);
    assert!(supervisor.snapshot().input_active);

    supervisor.set_microphone_access(MicrophoneAccess::Denied);
    assert_eq!(
        supervisor.snapshot().input_fault,
        Some(Fault::PermissionDenied)
    );
    supervisor.tick(33 * MS);
    render(&mut supervisor, 512);

    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running);
    assert!(!snapshot.input_active, "the microphone is released");
    assert_eq!(snapshot.input_fault, Some(Fault::PermissionDenied));
    assert_eq!(
        snapshot.engine.tone_hz,
        Some(220.0),
        "desired state was replayed"
    );
}

#[test]
fn a_granted_microphone_that_still_fails_is_blamed_on_the_device() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Granted);
    supervisor.backend_mut().fail_input = Some(refused_input());
    supervisor.start(true, 0);

    let snapshot = supervisor.snapshot();
    assert!(!snapshot.input_active);
    assert_eq!(snapshot.input_fault, Some(Fault::DeviceUnavailable));
    assert_eq!(snapshot.last_error, Some(refused_input()));
}

#[test]
fn the_permission_outlives_a_stop_and_a_restart() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Denied);
    supervisor.start(true, 0);
    supervisor.stop();
    assert_eq!(
        supervisor.snapshot().input_fault,
        None,
        "stop clears the fault"
    );

    supervisor.start(true, 10 * MS);
    assert_eq!(
        supervisor.snapshot().input_fault,
        Some(Fault::PermissionDenied)
    );
    assert_eq!(supervisor.backend_mut().input_opens, 0);
}

#[test]
fn a_denial_reported_before_the_microphone_is_wanted_still_names_the_permission() {
    // Found in review: the engine contract's order. The fault was stored only when the denial
    // arrived with the microphone already wanted, and start() wiped it.
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.start(false, 0);
    supervisor.set_microphone_access(MicrophoneAccess::Denied);
    supervisor.start(true, MS);
    supervisor.tick(33 * MS);

    let snapshot = supervisor.snapshot();
    assert_eq!(snapshot.state, SessionState::Running);
    assert!(!snapshot.input_active);
    assert_eq!(snapshot.input_fault, Some(Fault::PermissionDenied));
    assert_eq!(supervisor.backend_mut().input_opens, 0);

    // A retry, as the UI's "Retry microphone" sends, keeps saying why.
    supervisor.start(true, 66 * MS);
    supervisor.tick(99 * MS);
    assert_eq!(
        supervisor.snapshot().input_fault,
        Some(Fault::PermissionDenied)
    );
}

#[test]
fn a_new_grant_retries_a_microphone_that_failed_while_the_permission_was_unknown() {
    // Under Unknown, AAudio's -896 is most likely the missing permission, so the grant that
    // follows is worth one retry.
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.backend_mut().fail_input = Some(refused_input());
    supervisor.start(true, 0);
    assert_eq!(
        supervisor.snapshot().input_fault,
        Some(Fault::DeviceUnavailable)
    );

    supervisor.backend_mut().fail_input = None;
    supervisor.set_microphone_access(MicrophoneAccess::Granted);
    supervisor.tick(33 * MS);
    assert!(supervisor.snapshot().input_active);
}

#[test]
fn repeating_a_grant_is_not_a_retry_so_a_failing_microphone_cannot_loop() {
    let mut supervisor = Supervisor::new(Flaky::new());
    supervisor.set_microphone_access(MicrophoneAccess::Granted);
    supervisor.backend_mut().fail_input = Some(refused_input());
    supervisor.start(true, 0);
    let opens = supervisor.backend_mut().opens;

    for tick in 1..100 {
        supervisor.set_microphone_access(MicrophoneAccess::Granted);
        supervisor.tick(tick * 33 * MS);
    }
    assert_eq!(
        supervisor.backend_mut().opens,
        opens,
        "no reopen without a new fact"
    );
    assert_eq!(
        supervisor.snapshot().input_fault,
        Some(Fault::DeviceUnavailable)
    );
}
