import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:diapason/app.dart';
import 'package:diapason/flavor.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('starts on the tuner with the engine value rendered', (tester) async {
    final fake = FakeAudioEngine(
      status: const EngineStatus(
        dspBuild: 'diapason_dsp 0.1.0',
        engineBuild: 'diapason_engine 0.1.0',
      ),
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          engineHandleProvider.overrideWithValue(fake),
          microphonePermissionProvider.overrideWithValue(FakeMicrophonePermission()),
        ],
        child: const DiapasonApp(),
      ),
    );
    await tester.pumpAndSettle();

    // The router's initial location is the tuner, and the value it shows came from the engine.
    expect(find.byType(TunerScreen), findsOneWidget);
    expect(find.text('diapason_dsp 0.1.0'), findsOneWidget);
  });

  test('flavour defaults to dev when no --dart-define-from-file is supplied', () {
    // Tests run without the flavour defines, so this documents the fallback rather than the
    // configured value - the configured values are exercised by the flavour builds themselves.
    expect(Flavor.current, Flavor.dev);
    expect(Flavor.appTitle, 'Diapason (Dev)');
  });
}
