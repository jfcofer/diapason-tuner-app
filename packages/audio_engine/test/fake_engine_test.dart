import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the fake satisfies EngineHandle without loading a native library', () async {
    final engine = FakeAudioEngine();

    expect(engine, isA<EngineHandle>());
    await engine.initialize();

    expect(engine.initializeCount, 1);
    expect(engine.status().running, isFalse);
    expect(engine.status(), isA<EngineStatus>());
  });
}
