import 'package:core_platform/core_platform.dart';
import 'package:core_platform/src/platform_microphone_permission.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:permission_handler/permission_handler.dart' as plugin;

void main() {
  test('the fake settles on the configured status and counts requests', () async {
    final permission = FakeMicrophonePermission(onRequest: MicrophonePermissionStatus.denied);

    expect(await permission.status(), MicrophonePermissionStatus.notDetermined);
    expect(await permission.request(), MicrophonePermissionStatus.denied);
    expect(await permission.status(), MicrophonePermissionStatus.denied);
    expect(permission.requestCount, 1);
    expect(await permission.openSettings(), isTrue);
    expect(permission.settingsOpened, 1);
  });

  test('every plugin status maps to one of ours, and only final refusals are permanent', () {
    final mapped = {
      for (final status in plugin.PermissionStatus.values) status: fromPluginStatus(status),
    };

    expect(mapped[plugin.PermissionStatus.granted], MicrophonePermissionStatus.granted);
    expect(mapped[plugin.PermissionStatus.denied], MicrophonePermissionStatus.denied);
    expect(
      mapped[plugin.PermissionStatus.permanentlyDenied],
      MicrophonePermissionStatus.permanentlyDenied,
    );
    expect(
      mapped[plugin.PermissionStatus.restricted],
      MicrophonePermissionStatus.permanentlyDenied,
    );
    expect(mapped.values, isNot(contains(MicrophonePermissionStatus.notDetermined)));
  });
}
