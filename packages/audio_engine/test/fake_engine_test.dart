import 'package:audio_engine/audio_engine.dart';
import 'package:audio_engine/testing.dart';
import 'package:core_domain/core_domain.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the fake honours the engine contract with the microphone granted', () async {
    await verifyEngineContract(FakeAudioEngine());
  });

  test('the fake honours the engine contract with the microphone refused', () async {
    await verifyEngineContract(FakeAudioEngine(microphoneWorks: false), microphoneGranted: false);
  });

  test('a new listener gets the current state first, as from a live engine', () async {
    final engine = FakeAudioEngine()..start(input: true);
    final first = await engine.snapshots.first;
    expect(first.state, SessionState.running);
    expect(first.inputActive, isTrue);
  });

  test('the contract catches an engine that drops the microphone silently', () async {
    final broken = FakeAudioEngine(microphoneWorks: false);
    await expectLater(
      verifyEngineContract(broken, deadline: const Duration(milliseconds: 100)),
      throwsA(isA<EngineContractViolation>()),
    );
  });
}
