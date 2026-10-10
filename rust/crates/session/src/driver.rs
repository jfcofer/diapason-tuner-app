//! The thread that drives a [`Supervisor`] in real time.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use diapason_audio_io::AudioBackend;
use thiserror::Error;

use crate::{MicrophoneAccess, SessionSnapshot, Supervisor};

/// How often the supervisor checks the stream and the subscriber gets a snapshot: ~30 Hz, the
/// rate the UI is designed around (`docs/ARCHITECTURE.md` §4). The throttle lives here, in Rust.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(33);

/// Receives every snapshot on the session thread. Returns `false` to unsubscribe, for example
/// when the Dart stream it feeds has been cancelled.
type Subscriber = Box<dyn FnMut(&SessionSnapshot) -> bool + Send>;

enum Control {
    Start { input: bool },
    Stop,
    SetMicrophoneAccess(MicrophoneAccess),
    StartTone { frequency_hz: f32, amplitude: f32 },
    StopTone,
    Subscribe(Subscriber),
    Shutdown,
}

/// Why a [`Session`] could not do what it was asked.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// The operating system would not start the session thread.
    #[error("could not start the audio session thread")]
    Spawn,
    /// The session thread has exited, so nothing can reach the stream.
    #[error("the audio session has shut down")]
    Gone,
}

/// A running audio session: a [`Supervisor`] on its own thread (`docs/adr/0022`).
///
/// Every method returns at once. The thread applies the request, ticks the supervisor about every
/// 33 ms, and publishes a snapshot to the subscriber at the same rate.
///
/// Dropping the session stops the stream and joins the thread, so the drop blocks for as long as
/// the platform takes to stop: up to half a second on Android. Drop it only where that wait is
/// acceptable; the app holds one for its whole life.
pub struct Session {
    control: mpsc::Sender<Control>,
    thread: Option<JoinHandle<()>>,
}

impl Session {
    /// Start a session thread over `backend`, which must be closed. The stream does not open until
    /// [`start`](Self::start).
    ///
    /// # Errors
    /// [`SessionError::Spawn`] if the thread cannot be created.
    pub fn spawn<B: AudioBackend + Send + 'static>(backend: B) -> Result<Self, SessionError> {
        let (control, requests) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("diapason-session".to_owned())
            .spawn(move || run(Supervisor::new(backend), &requests))
            .map_err(|_| SessionError::Spawn)?;
        Ok(Self {
            control,
            thread: Some(thread),
        })
    }

    /// Run the stream, with the microphone if `input`. Asking for the microphone again retries it
    /// after an input fault.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn start(&self, input: bool) -> Result<(), SessionError> {
        self.send(Control::Start { input })
    }

    /// Close the stream.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn stop(&self) -> Result<(), SessionError> {
        self.send(Control::Stop)
    }

    /// Record the microphone permission; a running stream follows it on the next tick.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn set_microphone_access(&self, access: MicrophoneAccess) -> Result<(), SessionError> {
        self.send(Control::SetMicrophoneAccess(access))
    }

    /// Play a test tone, kept across rebuilds until stopped.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn start_tone(&self, frequency_hz: f32, amplitude: f32) -> Result<(), SessionError> {
        self.send(Control::StartTone {
            frequency_hz,
            amplitude,
        })
    }

    /// Stop the test tone.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn stop_tone(&self) -> Result<(), SessionError> {
        self.send(Control::StopTone)
    }

    /// Deliver snapshots to `subscriber`, replacing any earlier one. It runs on the session thread,
    /// so it must return quickly.
    ///
    /// # Errors
    /// [`SessionError::Gone`] if the session thread has exited.
    pub fn subscribe(
        &self,
        subscriber: impl FnMut(&SessionSnapshot) -> bool + Send + 'static,
    ) -> Result<(), SessionError> {
        self.send(Control::Subscribe(Box::new(subscriber)))
    }

    fn send(&self, request: Control) -> Result<(), SessionError> {
        self.control.send(request).map_err(|_| SessionError::Gone)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // If the thread has already gone, there is nothing to tell it.
        if self.control.send(Control::Shutdown).is_ok()
            && let Some(thread) = self.thread.take()
        {
            // A panicked thread already aborted in release (docs/adr/0021); in a debug build
            // there is nothing more to do with the panic than not propagate it from a destructor.
            let _ = thread.join();
        }
    }
}

fn run<B: AudioBackend>(mut supervisor: Supervisor<B>, requests: &mpsc::Receiver<Control>) {
    let epoch = Instant::now();
    let now_ns = || u64::try_from(epoch.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let mut subscriber: Option<Subscriber> = None;
    let mut next_publish = Instant::now();

    loop {
        let wait = next_publish.saturating_duration_since(Instant::now());
        match requests.recv_timeout(wait) {
            Ok(Control::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Control::Start { input }) => supervisor.start(input, now_ns()),
            Ok(Control::Stop) => supervisor.stop(),
            Ok(Control::SetMicrophoneAccess(access)) => supervisor.set_microphone_access(access),
            Ok(Control::StartTone {
                frequency_hz,
                amplitude,
            }) => supervisor.start_tone(frequency_hz, amplitude),
            Ok(Control::StopTone) => supervisor.stop_tone(),
            Ok(Control::Subscribe(new)) => subscriber = Some(new),
            Err(RecvTimeoutError::Timeout) => {}
        }

        if Instant::now() < next_publish {
            continue;
        }
        // From now, not from the missed deadline: a late tick must not cause a burst of catch-up.
        next_publish = Instant::now() + PUBLISH_INTERVAL;
        supervisor.tick(now_ns());
        if let Some(deliver) = &mut subscriber {
            let snapshot = supervisor.snapshot();
            if !deliver(&snapshot) {
                subscriber = None;
            }
        }
    }
    supervisor.stop();
}

#[cfg(test)]
mod tests {
    use diapason_audio_io::{BlockPattern, OfflineBackend};

    use super::*;
    use crate::SessionState;

    /// Long enough for any machine to schedule the thread; a deadline, not a measurement.
    const DEADLINE: Duration = Duration::from_secs(10);

    #[test]
    fn a_session_publishes_its_state_and_stops_cleanly_when_dropped() {
        let session = Session::spawn(OfflineBackend::new(BlockPattern::Fixed(256))).expect("spawn");
        let (sender, snapshots) = mpsc::channel();
        session
            .subscribe(move |snapshot| sender.send(snapshot.clone()).is_ok())
            .expect("subscribe");
        session.start(true).expect("start");

        let running = loop {
            let snapshot = snapshots.recv_timeout(DEADLINE).expect("a snapshot");
            if snapshot.state == SessionState::Running {
                break snapshot;
            }
        };
        assert!(running.input_active);
        assert_eq!(running.backend, "offline");
        // Dropping stops the stream and joins the thread; a hang here is the failure.
        drop(session);
    }

    /// The denied half of `verifyEngineContract` (`audio_engine`), in its exact order, on the
    /// real driver thread: the order that the device run would otherwise be first to try.
    #[test]
    fn the_engine_contracts_denied_order_reports_the_permission() {
        let session = Session::spawn(OfflineBackend::new(BlockPattern::Fixed(256))).expect("spawn");
        let (sender, snapshots) = mpsc::channel();
        session
            .subscribe(move |snapshot| sender.send(snapshot.clone()).is_ok())
            .expect("subscribe");
        // Snapshots keep coming, so the deadline bounds the whole wait, not each receive.
        let until = |holds: &dyn Fn(&SessionSnapshot) -> bool| {
            let started = Instant::now();
            loop {
                assert!(
                    started.elapsed() < DEADLINE,
                    "the snapshot awaited never came"
                );
                let snapshot = snapshots.recv_timeout(DEADLINE).expect("a snapshot");
                if holds(&snapshot) {
                    break snapshot;
                }
            }
        };

        session.start(false).expect("start");
        until(&|s| s.state == SessionState::Running);
        session
            .set_microphone_access(MicrophoneAccess::Denied)
            .expect("set access");
        session.start(true).expect("start with the microphone");
        let refused = until(&|s| s.input_fault.is_some());

        assert_eq!(refused.state, SessionState::Running);
        assert!(!refused.input_active);
        assert_eq!(refused.input_fault, Some(crate::Fault::PermissionDenied));
    }

    #[test]
    fn a_subscriber_that_returns_false_is_dropped() {
        let session = Session::spawn(OfflineBackend::new(BlockPattern::Fixed(256))).expect("spawn");
        let (sender, snapshots) = mpsc::channel();
        session
            .subscribe(move |_| {
                let _ = sender.send(());
                false
            })
            .expect("subscribe");
        snapshots.recv_timeout(DEADLINE).expect("one snapshot");
        assert_eq!(
            snapshots.recv_timeout(DEADLINE),
            Err(mpsc::RecvTimeoutError::Disconnected),
            "the subscriber was dropped after declining"
        );
    }
}
