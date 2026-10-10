// The FFI round trip on a real device (docs/TESTING.md §1): the same contract FakeAudioEngine
// passes in unit tests, here against the Rust engine and the platform backend. Run it through
// `just test-integration-android`, which runs it with the microphone revoked and then granted.
import 'package:audio_engine/audio_engine.dart';
import 'package:audio_engine/testing.dart';
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
    final engine = AudioEngine();
    await tester.runAsync(() => verifyEngineContract(engine, microphoneGranted: granted));

    // Record what the platform says when the microphone is refused, for the device log in the
    // task file. T-002b part 2b maps it to AudioFault.permissionDenied.
    if (!granted) {
      engine.start(input: true);
      final refused = await tester.runAsync(
        () => engine.snapshots.firstWhere((s) => s.inputFault != null),
      );
      debugPrint('microphone refused: ${refused?.inputFault} / ${refused?.diagnostics.lastError}');
      engine.stop();
    }
  });
}
