import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

/// The screen over fakes: no native library, no platform channel. Diagnostics are on, as in the
/// dev flavour, unless [diagnostics] says otherwise.
Future<FakeAudioEngine> pumpTuner(
  WidgetTester tester, {
  required FakeMicrophonePermission permission,
  FakeAudioEngine? engine,
  bool diagnostics = true,
}) async {
  final fake = engine ?? FakeAudioEngine();
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        engineHandleProvider.overrideWithValue(fake),
        microphonePermissionProvider.overrideWithValue(permission),
        showEngineDiagnosticsProvider.overrideWithValue(diagnostics),
      ],
      child: const MaterialApp(home: TunerScreen()),
    ),
  );
  await tester.pump();
  return fake;
}

Future<void> tapAndSettle(WidgetTester tester, Key key) async {
  await tester.tap(find.byKey(key));
  await tester.pump();
  await tester.pump();
}

String micMessage(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('tuner.micMessage'))).data!;

void main() {
  testWidgets('renders what the engine reports, with no native library loaded', (tester) async {
    await pumpTuner(
      tester,
      permission: FakeMicrophonePermission(),
      engine: FakeAudioEngine(
        status: const EngineStatus(
          dspBuild: 'diapason_dsp 9.9.9',
          engineBuild: 'diapason_engine 9.9.9',
        ),
      ),
    );
    expect(find.byKey(const Key('tuner.dspBuild')), findsOneWidget);
    expect(find.text('diapason_dsp 9.9.9'), findsOneWidget);
  });

  testWidgets('explains the microphone before asking for it', (tester) async {
    final permission = FakeMicrophonePermission();
    await pumpTuner(tester, permission: permission);

    expect(micMessage(tester), contains('nothing is recorded or sent'));
    expect(permission.requestCount, 0, reason: 'no prompt until the user chooses to listen');
  });

  testWidgets('once granted, the stream opens with the microphone', (tester) async {
    final permission = FakeMicrophonePermission();
    final engine = await pumpTuner(tester, permission: permission);

    await tapAndSettle(tester, const Key('tuner.micAction'));

    expect(permission.requestCount, 1);
    expect(engine.current.state, SessionState.running);
    expect(engine.current.inputActive, isTrue);
    expect(find.byKey(const Key('tuner.micMessage')), findsNothing);
    expect(find.textContaining('Microphone: silence'), findsOneWidget);
  });

  testWidgets('a refusal opens no stream and offers to ask again', (tester) async {
    final permission = FakeMicrophonePermission(onRequest: MicrophonePermissionStatus.denied);
    final engine = await pumpTuner(tester, permission: permission);

    await tapAndSettle(tester, const Key('tuner.micAction'));

    expect(engine.current.state, SessionState.stopped);
    expect(micMessage(tester), contains('needs the microphone'));
    expect(find.text('Try again'), findsOneWidget);
  });

  testWidgets('a refusal is told to the engine, which then never opens the microphone', (
    tester,
  ) async {
    final permission = FakeMicrophonePermission(onRequest: MicrophonePermissionStatus.denied);
    final engine = await pumpTuner(tester, permission: permission);
    await tapAndSettle(tester, const Key('tuner.micAction'));

    engine.start(input: true);
    expect(engine.current.inputActive, isFalse);
    expect(engine.current.inputFault, AudioFault.permissionDenied);
  });

  testWidgets('with diagnostics on, the preset requested and obtained are shown', (tester) async {
    await pumpTuner(tester, permission: FakeMicrophonePermission());
    await tapAndSettle(tester, const Key('tuner.micAction'));
    expect(
      find.text('Preset: voiceRecognition requested, voiceRecognition obtained'),
      findsOneWidget,
    );
  });

  testWidgets('without diagnostics, no test tone and no preset readout', (tester) async {
    await pumpTuner(tester, permission: FakeMicrophonePermission(), diagnostics: false);
    await tapAndSettle(tester, const Key('tuner.micAction'));
    expect(find.byKey(const Key('tuner.testTone')), findsNothing);
    expect(find.textContaining('Preset:'), findsNothing);
  });

  testWidgets('a permanent refusal sends the user to Settings, without prompting', (tester) async {
    final permission = FakeMicrophonePermission(
      initial: MicrophonePermissionStatus.permanentlyDenied,
      onRequest: MicrophonePermissionStatus.permanentlyDenied,
    );
    await pumpTuner(tester, permission: permission);
    await tapAndSettle(tester, const Key('tuner.micAction'));
    expect(permission.requestCount, 0, reason: 'the OS would not show the prompt anyway');
    expect(micMessage(tester), contains('Turn it on in Settings'));

    await tapAndSettle(tester, const Key('tuner.micAction'));
    expect(permission.settingsOpened, 1);
  });

  testWidgets('the test tone plays with the microphone refused, as the metronome must', (
    tester,
  ) async {
    final permission = FakeMicrophonePermission(onRequest: MicrophonePermissionStatus.denied);
    final engine = await pumpTuner(tester, permission: permission);
    await tapAndSettle(tester, const Key('tuner.micAction'));

    await tapAndSettle(tester, const Key('tuner.testTone'));
    expect(engine.current.state, SessionState.running);
    expect(engine.current.inputActive, isFalse);
    expect(engine.current.toneHz, 440);
    expect(find.text('Stop test tone'), findsOneWidget);

    await tapAndSettle(tester, const Key('tuner.testTone'));
    expect(engine.current.toneHz, isNull);
  });

  testWidgets('faults and route changes are shown, not swallowed', (tester) async {
    final engine = await pumpTuner(tester, permission: FakeMicrophonePermission());
    engine.emit(
      const SessionSnapshot(
        state: SessionState.running,
        sampleRate: 48000,
        rebuilds: 2,
        inputFault: AudioFault.deviceUnavailable,
        diagnostics: StreamDiagnostics(backend: 'fake'),
      ),
    );
    await tester.pump();
    expect(find.text('Microphone unavailable: deviceUnavailable'), findsOneWidget);
    expect(find.text('Audio route changed 2×'), findsOneWidget);
  });

  testWidgets('a microphone fault with the permission granted offers a retry', (tester) async {
    final engine = await pumpTuner(
      tester,
      permission: FakeMicrophonePermission(),
      engine: FakeAudioEngine(microphoneWorks: false),
    );
    await tapAndSettle(tester, const Key('tuner.micAction'));
    expect(engine.current.inputFault, AudioFault.deviceUnavailable);

    engine.microphoneWorks = true;
    await tapAndSettle(tester, const Key('tuner.retryMic'));
    expect(engine.current.inputActive, isTrue);
    expect(find.byKey(const Key('tuner.retryMic')), findsNothing);
  });
}
