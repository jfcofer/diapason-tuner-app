import 'package:audio_engine/audio_engine.dart';
import 'package:audio_engine/testing.dart';
import 'package:core_domain/core_domain.dart';
import 'package:flutter_test/flutter_test.dart';

/// An engine that never hears about the permission, so it blames the device for a refused
/// microphone: what the real engine did before `T-002b` part 2b.
class _IgnoresPermission extends FakeAudioEngine {
  new() : super(microphoneWorks: false);

  @override
  void setMicrophoneAccess({required bool granted}) {}
}

void main() {
  test('the fake honours the engine contract with the microphone granted', () async {
    await verifyEngineContract(FakeAudioEngine(), reportsInputPreset: true);
  });

  test('the fake honours the engine contract with the microphone refused', () async {
    await verifyEngineContract(FakeAudioEngine(), microphoneGranted: false);
  });

  test('a new listener gets the current state at once', () async {
    final engine = FakeAudioEngine()..start(input: true);
    final first = await engine.snapshots.first;
    expect(first.state, SessionState.running);
    expect(first.inputActive, isTrue);
  });

  test('a permitted microphone the device refuses is blamed on the device', () {
    final engine = FakeAudioEngine(microphoneWorks: false)
      ..setMicrophoneAccess(granted: true)
      ..start(input: true);
    expect(engine.current.inputActive, isFalse);
    expect(engine.current.inputFault, AudioFault.deviceUnavailable);
  });

  test('granting the microphone to a running stream brings it back', () {
    final engine = FakeAudioEngine()
      ..setMicrophoneAccess(granted: false)
      ..start(input: true);
    expect(engine.current.inputFault, AudioFault.permissionDenied);

    engine.setMicrophoneAccess(granted: true);
    expect(engine.current.inputActive, isTrue);
    expect(engine.current.inputFault, isNull);
  });

  test('repeating a grant retries nothing, as in the real session', () {
    final engine = FakeAudioEngine(microphoneWorks: false)
      ..setMicrophoneAccess(granted: true)
      ..start(input: true);
    expect(engine.current.inputFault, AudioFault.deviceUnavailable);

    engine
      ..microphoneWorks = true
      ..setMicrophoneAccess(granted: true);
    expect(engine.current.inputActive, isFalse, reason: 'no new fact, no retry');
  });

  test('the contract catches an engine that drops the microphone silently', () async {
    final broken = FakeAudioEngine(microphoneWorks: false);
    await expectLater(
      verifyEngineContract(broken, deadline: const Duration(milliseconds: 100)),
      throwsA(isA<EngineContractViolation>()),
    );
  });

  test('the contract catches a refused microphone reported as a broken device', () async {
    await expectLater(
      verifyEngineContract(
        _IgnoresPermission(),
        microphoneGranted: false,
        deadline: const Duration(milliseconds: 100),
      ),
      throwsA(isA<EngineContractViolation>()),
    );
  });
}
