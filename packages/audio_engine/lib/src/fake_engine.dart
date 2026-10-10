import 'dart:async';

import 'package:audio_engine/src/engine_facade.dart';
import 'package:core_domain/core_domain.dart';

/// In-memory [EngineHandle] used by every UI test.
///
/// Exists so that a widget test never loads a native library: goldens and widget tests must run in
/// the pinned container with no Rust build in sight (`docs/TESTING.md`). It honours the same
/// contract as the real engine (`verifyEngineContract`), and applies commands at once.
class FakeAudioEngine implements EngineHandle {
  /// Creates a fake reporting [status]. It follows the real session's rules: a microphone whose
  /// permission was refused ([setMicrophoneAccess]) is never opened and reports
  /// [AudioFault.permissionDenied]; with [microphoneWorks] false, a permitted microphone fails as
  /// a broken device would, with [AudioFault.deviceUnavailable].
  new({EngineStatus? status, this.microphoneWorks = true})
    : _status =
          status ??
          const EngineStatus(
            dspBuild: 'diapason_dsp 0.1.0 (fake)',
            engineBuild: 'diapason_engine 0.1.0 (fake)',
          );

  final EngineStatus _status;
  final StreamController<SessionSnapshot> _controller = StreamController.broadcast();

  /// Whether a permitted microphone opens. False plays a device that refuses it.
  bool microphoneWorks;

  /// The permission as last reported: `null` until [setMicrophoneAccess] is called.
  bool? _microphoneGranted;

  /// Whether the microphone was asked for by the last [start].
  bool _wantsInput = false;

  /// How many times [initialize] has been called.
  int initializeCount = 0;

  /// The latest snapshot, which every new listener of [snapshots] receives first. The real engine
  /// publishes every ~33 ms, so its listeners get the current state almost as promptly.
  SessionSnapshot current = const SessionSnapshot(
    state: SessionState.stopped,
    sampleRate: 0,
    diagnostics: StreamDiagnostics(backend: 'fake'),
  );

  @override
  Future<void> initialize() async => initializeCount++;

  @override
  EngineStatus status() => _status;

  @override
  Stream<SessionSnapshot> get snapshots async* {
    yield current;
    yield* _controller.stream;
  }

  /// The tone asked for, as the real engine's desired state: it sounds only while a stream runs.
  double? _toneHz;

  @override
  void start({required bool input}) {
    _wantsInput = input;
    _publishRunning();
  }

  @override
  void setMicrophoneAccess({required bool granted}) {
    final repeatedGrant = granted && _microphoneGranted == true;
    _microphoneGranted = granted;
    // The real session follows a change on its next tick; the fake applies it at once. Repeating
    // a grant is not a new fact, so, as in the real session, it retries nothing.
    if (current.state == SessionState.running && !repeatedGrant) _publishRunning();
  }

  void _publishRunning() {
    final AudioFault? inputFault;
    if (!_wantsInput) {
      inputFault = null;
    } else if (_microphoneGranted == false) {
      inputFault = AudioFault.permissionDenied;
    } else if (!microphoneWorks) {
      inputFault = AudioFault.deviceUnavailable;
    } else {
      inputFault = null;
    }
    final micOpen = _wantsInput && inputFault == null;
    _publish(
      SessionSnapshot(
        state: SessionState.running,
        sampleRate: 48000,
        inputActive: micOpen,
        toneHz: _toneHz,
        rebuilds: current.rebuilds,
        inputFault: inputFault,
        diagnostics: StreamDiagnostics(
          backend: 'fake',
          maxBlockFrames: 1024,
          requestedInputPreset: micOpen ? InputPreset.voiceRecognition : null,
          inputPreset: micOpen ? InputPreset.voiceRecognition : null,
        ),
      ),
    );
  }

  @override
  void stop() {
    _wantsInput = false;
    _publishStopped();
  }

  void _publishStopped() => _publish(
    SessionSnapshot(
      state: SessionState.stopped,
      sampleRate: 0,
      rebuilds: current.rebuilds,
      diagnostics: const StreamDiagnostics(backend: 'fake'),
    ),
  );

  @override
  void startTone({required double frequencyHz, required double amplitude}) {
    _toneHz = frequencyHz;
    _publishTone();
  }

  @override
  void stopTone() {
    _toneHz = null;
    _publishTone();
  }

  void _publishTone() => _publish(
    SessionSnapshot(
      state: current.state,
      sampleRate: current.sampleRate,
      inputActive: current.inputActive,
      inputRms: current.inputRms,
      toneHz: current.state == SessionState.running ? _toneHz : null,
      rebuilds: current.rebuilds,
      fault: current.fault,
      inputFault: current.inputFault,
      diagnostics: current.diagnostics,
    ),
  );

  /// Publish [snapshot] as if the engine had, for tests that need a particular state: a fault,
  /// a rebuild, a level.
  void emit(SessionSnapshot snapshot) => _publish(snapshot);

  void _publish(SessionSnapshot snapshot) {
    current = snapshot;
    _controller.add(snapshot);
  }
}
