// The tuner's rows of the lifecycle matrix (docs/PLATFORM_AUDIO.md §5) that patrol can drive: the
// app as it ships, the real engine, and the OS pressing Home and recents. Test names carry the
// matrix IDs, which `docs-check` matches against the matrix. Run through
// `just test-integration-android`; `just mic-clients` shows what the OS itself sees.
//
// Each test starts from cleared app data (docs/adr/0026), so the microphone is undecided and the
// test answers the dialog. Every wait is bounded by snapshots or a timeout, never a measured time
// (docs/TESTING.md §4).
import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:diapason/bootstrap.dart';
import 'package:diapason/router.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:patrol/patrol.dart';

/// The session publishes about 30 times a second, so the screen never settles: pump, never settle.
const _config = PatrolTesterConfig(printLogs: true, settlePolicy: SettlePolicy.noSettle);

/// Generous: a stream reopens in well under a second, and this only bounds a broken run.
const _deadline = Duration(seconds: 10);

void main() {
  patrolTest('L1 L2: Home releases the microphone, and reopening listens again', config: _config, (
    $,
  ) async {
    final engine = await _listening($);

    await $.platform.mobile.pressHome();
    await _until(engine, 'the stream stops in the background', (s) => !s.inputActive);

    await $.platform.mobile.openApp();
    await _until(engine, 'listening resumes in the foreground', (s) => s.inputActive);
  });

  patrolTest(
    'L1 L2: recents release the microphone, and reopening listens again',
    config: _config,
    ($) async {
      final engine = await _listening($);

      await $.platform.mobile.pressRecentApps();
      await _until(engine, 'the stream stops behind recents', (s) => !s.inputActive);

      await $.platform.mobile.openApp();
      await _until(engine, 'listening resumes in the foreground', (s) => s.inputActive);
    },
  );

  patrolTest(
    'L11: leaving the tuner releases the microphone, and returning does not reopen it',
    config: _config,
    ($) async {
      final engine = await _listening($);

      appRouter.go(MetronomeRoute.path);
      await $.pump();
      await _until(
        engine,
        'the stream stops off the tuner',
        (s) => s.state == SessionState.stopped,
      );

      appRouter.go(TunerRoute.path);
      await $.pump();
      // About half a second of snapshots, counted rather than timed: none may open the microphone.
      final next = await engine.snapshots.take(15).toList().timeout(_deadline);
      expect(next.where((s) => s.inputActive), isEmpty, reason: 'the tuner listened unasked');
      expect($(const Key('tuner.listenToggle')), findsOneWidget);
    },
  );
}

/// The app as it ships, listening, with the microphone allowed through the real dialog.
Future<AudioEngine> _listening(PatrolIntegrationTester $) async {
  final engine = AudioEngine(capabilities: await PlatformAudioCapabilities().read());
  await $.pumpWidget(await diapasonRoot(engine: engine));
  await $(const Key('tuner.micAction')).tap();
  expect(
    await $.platform.mobile.isPermissionDialogVisible(timeout: _deadline),
    isTrue,
    reason: 'no microphone dialog: was the app data cleared before this test (clearPackageData)?',
  );
  await $.platform.mobile.grantPermissionWhenInUse();
  await _until(engine, 'the microphone opens once allowed', (s) => s.inputActive);
  return engine;
}

/// Waits for the first snapshot that satisfies [done], failing with [what] after [_deadline].
Future<void> _until(AudioEngine engine, String what, bool Function(SessionSnapshot) done) async {
  final reached = await engine.snapshots
      .firstWhere(done)
      .then((_) => true)
      .timeout(_deadline, onTimeout: () => false);
  expect(reached, isTrue, reason: 'timed out waiting until $what');
}
