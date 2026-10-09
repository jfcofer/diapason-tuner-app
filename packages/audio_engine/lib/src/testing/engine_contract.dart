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
/// two cannot drift apart. With [microphoneGranted] false, asking for the microphone must keep the
/// stream running without it and report an input fault. [deadline] bounds each wait: it is a
/// timeout, never a measurement.
Future<void> verifyEngineContract(
  EngineHandle engine, {
  bool microphoneGranted = true,
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

  engine.start(input: true);
  if (microphoneGranted) {
    await until('the microphone open', (s) => s.state == SessionState.running && s.inputActive);
  } else {
    await until(
      'output running without the microphone, with an input fault',
      (s) => s.state == SessionState.running && !s.inputActive && s.inputFault != null,
    );
  }

  engine
    ..startTone(frequencyHz: 220, amplitude: 0.1)
    ..stop();
  await until(
    'the stream stopped, with no tone',
    (s) => s.state == SessionState.stopped && s.toneHz == null,
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
