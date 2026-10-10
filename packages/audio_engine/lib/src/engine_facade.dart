import 'dart:async';

import 'package:audio_engine/src/rust/api/session.dart' as ffi;
import 'package:audio_engine/src/rust/api/simple.dart' as ffi;
import 'package:audio_engine/src/rust/frb_generated.dart';
import 'package:core_domain/core_domain.dart';

/// Talks to the Rust engine: commands in, [SessionSnapshot]s out (`docs/ARCHITECTURE.md` §4).
///
/// Implementations: [AudioEngine] over the real bridge, and `FakeAudioEngine` in memory. Both
/// must pass `verifyEngineContract` (`package:audio_engine/testing.dart`). Features and widget
/// tests depend on this interface, never on the generated bindings, so a UI test never loads a
/// native library.
///
/// Every command returns at once; its effect shows up in a later snapshot.
abstract interface class EngineHandle {
  /// Prepare the engine. Must complete before any other call.
  Future<void> initialize();

  /// Which builds are running.
  EngineStatus status();

  /// The session about 30 times a second, whether or not a stream is open. Broadcast: a new
  /// listener gets the current state within one period (the fake replays its latest at once).
  Stream<SessionSnapshot> get snapshots;

  /// Run the stream, with the microphone if [input]. Asking for the microphone again retries it
  /// after an input fault. Only ask for it once the permission is granted.
  void start({required bool input});

  /// Close the stream.
  void stop();

  /// Play a test tone at [frequencyHz] with peak [amplitude] in 0 to 1, kept across rebuilds.
  void startTone({required double frequencyHz, required double amplitude});

  /// Stop the test tone.
  void stopTone();
}

/// The real engine, over the flutter_rust_bridge boundary.
class AudioEngine implements EngineHandle {
  /// Creates a handle. Call [initialize] before anything else.
  new();

  ffi.AudioSession? _session;
  Stream<SessionSnapshot>? _snapshots;

  ffi.AudioSession get _live {
    final session = _session;
    if (session == null) throw StateError('call initialize() before using the engine');
    return session;
  }

  @override
  Future<void> initialize() async {
    if (_session != null) return;
    await RustLib.init();
    final session = ffi.AudioSession.spawn();
    _session = session;
    // Subscribe once, for the life of the app, and share it: Rust keeps one subscriber.
    _snapshots = session.snapshots().map(_toDomain).asBroadcastStream();
  }

  @override
  EngineStatus status() {
    final status = ffi.engineStatus();
    // Map the generated type into the domain type here, at the boundary, so nothing above this
    // layer holds a bridge-generated object.
    return EngineStatus(dspBuild: status.dspBuild, engineBuild: status.engineBuild);
  }

  @override
  Stream<SessionSnapshot> get snapshots =>
      _snapshots ?? (throw StateError('call initialize() before using the engine'));

  @override
  void start({required bool input}) => _live.start(input: input);

  @override
  void stop() => _live.stop();

  @override
  void startTone({required double frequencyHz, required double amplitude}) =>
      _live.startTone(frequencyHz: frequencyHz, amplitude: amplitude);

  @override
  void stopTone() => _live.stopTone();
}

SessionSnapshot _toDomain(ffi.SessionSnapshotDto dto) => SessionSnapshot(
  state: switch (dto.state) {
    ffi.SessionStateDto.stopped => SessionState.stopped,
    ffi.SessionStateDto.running => SessionState.running,
    ffi.SessionStateDto.recovering => SessionState.recovering,
    ffi.SessionStateDto.failed => SessionState.failed,
  },
  sampleRate: dto.sampleRate,
  inputActive: dto.inputActive,
  inputRms: dto.inputRms,
  toneHz: dto.toneHz,
  rebuilds: dto.rebuilds,
  fault: _fault(dto.fault),
  inputFault: _fault(dto.inputFault),
  diagnostics: StreamDiagnostics(
    backend: dto.backend,
    maxBlockFrames: dto.maxBlockFrames,
    framesPlayed: dto.frames,
    callbacks: dto.callbacks,
    worstCallback: Duration(microseconds: dto.worstCallbackNs ~/ 1000),
    inputUnderruns: dto.inputUnderruns,
    xruns: dto.xruns,
    framesPerBurst: dto.framesPerBurst,
    inputPreset: switch (dto.inputPreset) {
      null => null,
      ffi.InputPresetDto.unprocessed => InputPreset.unprocessed,
      ffi.InputPresetDto.voiceRecognition => InputPreset.voiceRecognition,
      ffi.InputPresetDto.other => InputPreset.other,
    },
    inputPresetCode: dto.inputPresetCode,
    outputPath: _path(dto.outputLowLatency, dto.outputExclusive),
    inputPath: _path(dto.inputLowLatency, dto.inputExclusive),
    lastError: dto.lastError,
    commandsDropped: dto.commandsDropped,
  ),
);

AudioFault? _fault(ffi.FaultDto? fault) => switch (fault) {
  null => null,
  ffi.FaultDto.permissionDenied => AudioFault.permissionDenied,
  ffi.FaultDto.deviceUnavailable => AudioFault.deviceUnavailable,
  ffi.FaultDto.configurationUnsupported => AudioFault.configurationUnsupported,
  ffi.FaultDto.internal => AudioFault.internal,
};

GrantedPath? _path(bool? lowLatency, bool? exclusive) => lowLatency == null || exclusive == null
    ? null
    : GrantedPath(lowLatency: lowLatency, exclusive: exclusive);
