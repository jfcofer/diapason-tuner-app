import 'dart:async';

import 'package:audio_engine/src/engine_facade.dart';
import 'package:core_domain/core_domain.dart';

/// A rule of the [EngineHandle] contract that an implementation broke.
class EngineContractViolation implements Exception {
  /// Creates a violation described by [message].
  const new(this.message);

  /// What was expected and did not happen.
  final String message;

  @override
  String toString() => 'EngineContractViolation: $message';
}

/// Drive [engine] through the [EngineHandle] contract, throwing [EngineContractViolation] on the
/// first rule it breaks (`docs/TESTING.md` §3: the fake is a contract, not a stub).
///
/// Runs against `FakeAudioEngine` in unit tests and against the real engine on a device, so the
/// two cannot drift apart. [microphoneGranted] is the real permission, which the engine is told:
/// refused, asking for the microphone must keep the stream running without it and report
/// [AudioFault.permissionDenied]. With [reportsInputPreset], an open microphone must report the
/// input preset requested and obtained: true on Android, where presets exist. [deadline] bounds
/// each wait: it is a timeout, never a measurement.
Future<void> verifyEngineContract(
  EngineHandle engine, {
  bool microphoneGranted = true,
  bool reportsInputPreset = false,
  Duration deadline = const Duration(seconds: 10),
}) async {
  await engine.initialize();
  final status = engine.status();
  if (status.dspBuild.isEmpty || status.engineBuild.isEmpty) {
    throw EngineContractViolation('status() reported an empty build: $status');
  }

  Future<SessionSnapshot> until(String what, bool Function(SessionSnapshot) holds) => engine
      .snapshots
      .firstWhere(holds)
      .timeout(deadline, onTimeout: () => throw EngineContractViolation('never saw: $what'));

  engine.start(input: false);
  await until(
    'an output-only stream running',
    (s) => s.state == SessionState.running && !s.inputActive,
  );

  engine.startTone(frequencyHz: 440, amplitude: 0.1);
  await until('the tone playing at 440 Hz', (s) => s.toneHz == 440);
  engine.stopTone();
  await until('the tone stopped', (s) => s.toneHz == null);

  engine
    ..setMicrophoneAccess(granted: microphoneGranted)
    ..start(input: true);
  if (microphoneGranted) {
    await until(
      reportsInputPreset
          ? 'the microphone open, with the preset requested and obtained reported'
          : 'the microphone open',
      (s) =>
          s.state == SessionState.running &&
          s.inputActive &&
          (!reportsInputPreset ||
              s.diagnostics.requestedInputPreset != null && s.diagnostics.inputPreset != null),
    );
  } else {
    await until(
      'output running without the microphone, the fault saying the permission is missing',
      (s) =>
          s.state == SessionState.running &&
          !s.inputActive &&
          s.inputFault == AudioFault.permissionDenied,
    );
  }

  engine
    ..startTone(frequencyHz: 220, amplitude: 0.1)
    ..stop();
  await until(
    'the stream stopped, with no tone, no rate and no stale microphone fault',
    (s) =>
        s.state == SessionState.stopped &&
        s.toneHz == null &&
        s.sampleRate == 0 &&
        s.inputFault == null,
  );
  engine.start(input: false);
  await until(
    'the tone replayed into the new stream',
    (s) => s.state == SessionState.running && s.toneHz == 220,
  );
  engine
    ..stopTone()
    ..stop();
  await until('the stream stopped', (s) => s.state == SessionState.stopped);
}
