// The FFI round trip on a real device (docs/TESTING.md §1): the same contract FakeAudioEngine
// passes in unit tests, here against the Rust engine and the platform backend. Run it through
// `just test-integration-android`, which runs it with the microphone revoked and then granted.
import 'package:audio_engine/audio_engine.dart';
import 'package:audio_engine/testing.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

/// What the driving script arranged: `granted` or `denied`; `request`, which shows the system
/// prompt for a person to allow; or empty when run by hand, which takes the permission as it is.
const expectedMicrophone = String.fromEnvironment('DIAPASON_MIC');

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the real engine honours the engine contract on this device', (tester) async {
    const permission = PlatformMicrophonePermission();
    final status = expectedMicrophone == 'request'
        ? await tester.runAsync(permission.request)
        : await permission.status();
    final granted = status == MicrophonePermissionStatus.granted;
    if (expectedMicrophone == 'request') {
      expect(granted, isTrue, reason: 'the prompt was not allowed on the device');
    } else if (expectedMicrophone.isNotEmpty) {
      expect(
        granted,
        expectedMicrophone == 'granted',
        reason:
            'expected the microphone $expectedMicrophone; set it so on the device and run again',
      );
    }
    final capabilities = await tester.runAsync(PlatformAudioCapabilities().read);
    debugPrint('device capabilities: $capabilities');
    final engine = AudioEngine(capabilities: capabilities ?? AudioDeviceCapabilities.unknown);
    await tester.runAsync(
      () => verifyEngineContract(
        engine,
        microphoneGranted: granted,
        reportsInputPreset: defaultTargetPlatform == TargetPlatform.android,
      ),
    );

    // Record what the device gives the microphone, for the device log in the task file: the preset
    // requested and obtained, or, refused, the fault (the engine no longer asks the platform).
    engine
      ..setMicrophoneAccess(granted: granted)
      ..start(input: true);
    final settled = await tester.runAsync(
      () => engine.snapshots.firstWhere((s) => s.inputActive || s.inputFault != null),
    );
    final diagnostics = settled?.diagnostics;
    debugPrint(
      'microphone: active ${settled?.inputActive}, fault ${settled?.inputFault}, preset '
      '${diagnostics?.requestedInputPreset} requested, ${diagnostics?.inputPreset} obtained '
      '(code ${diagnostics?.inputPresetCode}), input path ${diagnostics?.inputPath}, '
      '${settled?.sampleRate} Hz, burst ${diagnostics?.framesPerBurst}',
    );
    engine.stop();
  });
}
