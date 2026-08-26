import 'package:core_platform/core_platform.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the fake settles on the configured status and counts requests', () async {
    final permission = FakeMicrophonePermission(onRequest: MicrophonePermissionStatus.denied);

    expect(await permission.status(), MicrophonePermissionStatus.notDetermined);
    expect(await permission.request(), MicrophonePermissionStatus.denied);
    expect(await permission.status(), MicrophonePermissionStatus.denied);
    expect(permission.requestCount, 1);
  });
}
