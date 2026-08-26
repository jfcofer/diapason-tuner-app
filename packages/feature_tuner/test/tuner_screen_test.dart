import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('renders what the engine reports, with no native library loaded', (tester) async {
    final fake = FakeAudioEngine(
      status: const EngineStatus(
        dspBuild: 'diapason_dsp 9.9.9',
        engineBuild: 'diapason_engine 9.9.9',
        running: false,
      ),
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [engineHandleProvider.overrideWithValue(fake)],
        child: const MaterialApp(home: TunerScreen()),
      ),
    );

    expect(find.byKey(const Key('tuner.dspBuild')), findsOneWidget);
    expect(find.text('diapason_dsp 9.9.9'), findsOneWidget);
    expect(find.text('no stream yet (T-002)'), findsOneWidget);
  });
}
