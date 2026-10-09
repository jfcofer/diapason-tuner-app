import 'dart:async';

import 'package:audio_engine/src/engine_facade.dart';
import 'package:core_domain/core_domain.dart';

/// In-memory [EngineHandle] used by every UI test.
///
/// Exists so that a widget test never loads a native library: goldens and widget tests must run in
/// the pinned container with no Rust build in sight (`docs/TESTING.md`). It honours the same
/// contract as the real engine (`verifyEngineContract`), and applies commands at once.
class FakeAudioEngine implements EngineHandle {
  /// Creates a fake reporting [status]. With [microphoneWorks] false, asking for the microphone
  /// runs the stream without it and reports [inputFaultWhenBroken], as the real engine does.
  new({
    EngineStatus? status,
    this.microphoneWorks = true,
    this.inputFaultWhenBroken = AudioFault.permissionDenied,
  }) : _status =
           status ??
           const EngineStatus(
             dspBuild: 'diapason_dsp 0.1.0 (fake)',
             engineBuild: 'diapason_engine 0.1.0 (fake)',
           );

  final EngineStatus _status;
  final StreamController<SessionSnapshot> _controller = StreamController.broadcast();

  /// Whether a request for the microphone succeeds.
  bool microphoneWorks;

  /// The input fault reported when the microphone does not work.
  final AudioFault inputFaultWhenBroken;

  /// How many times [initialize] has been called.
  int initializeCount = 0;

  /// The latest snapshot, which every new listener of [snapshots] receives first.
  SessionSnapshot current = const SessionSnapshot(
    state: SessionState.stopped,
    sampleRate: 48000,
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
    final micOpen = input && microphoneWorks;
    _publish(
      SessionSnapshot(
        state: SessionState.running,
        sampleRate: current.sampleRate,
        inputActive: micOpen,
        toneHz: _toneHz,
        rebuilds: current.rebuilds,
        inputFault: input && !micOpen ? inputFaultWhenBroken : null,
        diagnostics: const StreamDiagnostics(backend: 'fake', maxBlockFrames: 1024),
      ),
    );
  }

  @override
  void stop() => _publish(
    SessionSnapshot(
      state: SessionState.stopped,
      sampleRate: current.sampleRate,
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
