// The FFI round trip on a real device (docs/TESTING.md §1): the same contract FakeAudioEngine
// passes in unit tests, here against the Rust engine and the platform backend. Run it through
// `just test-integration-android`. Each test starts from cleared app data (docs/adr/0026), so the
// microphone is undecided, and patrol answers the system dialog the way the test needs.
import 'package:audio_engine/audio_engine.dart';
import 'package:audio_engine/testing.dart';
import 'package:core_platform/core_platform.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:patrol/patrol.dart';

void main() {
  patrolTest('without the microphone, the output runs and the input reports why', ($) async {
    final granted = await _answerMicrophone($, allow: false);
    expect(granted, isFalse, reason: 'the dialog was not denied');
    await _verifyRealEngine(granted: false);
  });

  patrolTest('with the microphone, the duplex stream opens', ($) async {
    final granted = await _answerMicrophone($, allow: true);
    expect(granted, isTrue, reason: 'the dialog was not allowed');
    await _verifyRealEngine(granted: true);
  });
}

/// Asks for the microphone as the tuner does, and answers the system dialog.
Future<bool> _answerMicrophone(PatrolIntegrationTester $, {required bool allow}) async {
  const permission = PlatformMicrophonePermission();
  final answer = permission.request();
  // A test starts from cleared data, so the dialog must appear. If it does not, the isolation that
  // every test here relies on is gone, and that is the failure to report.
  expect(
    await $.platform.mobile.isPermissionDialogVisible(timeout: const Duration(seconds: 10)),
    isTrue,
    reason: 'no microphone dialog: was the app data cleared before this test (clearPackageData)?',
  );
  if (allow) {
    await $.platform.mobile.grantPermissionWhenInUse();
  } else {
    await $.platform.mobile.denyPermission();
  }
  return await answer == MicrophonePermissionStatus.granted;
}

Future<void> _verifyRealEngine({required bool granted}) async {
  final capabilities = await PlatformAudioCapabilities().read();
  debugPrint('device capabilities: $capabilities');
  final engine = AudioEngine(capabilities: capabilities);
  await verifyEngineContract(
    engine,
    microphoneGranted: granted,
    reportsInputPreset: defaultTargetPlatform == TargetPlatform.android,
  );

  // Record what the device gives the microphone, for the device log in the task file: the preset
  // requested and obtained, or, refused, the fault (the engine no longer asks the platform).
  engine
    ..setMicrophoneAccess(granted: granted)
    ..start(input: true);
  final settled = await engine.snapshots.firstWhere((s) => s.inputActive || s.inputFault != null);
  final diagnostics = settled.diagnostics;
  debugPrint(
    'microphone: active ${settled.inputActive}, fault ${settled.inputFault}, preset '
    '${diagnostics.requestedInputPreset} requested, ${diagnostics.inputPreset} obtained '
    '(code ${diagnostics.inputPresetCode}), input path ${diagnostics.inputPath}, '
    '${settled.sampleRate} Hz, burst ${diagnostics.framesPerBurst}',
  );
  engine.stop();
}
