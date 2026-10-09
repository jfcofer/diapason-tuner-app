import 'package:core_platform/src/microphone_permission.dart';
import 'package:flutter/foundation.dart';
import 'package:permission_handler/permission_handler.dart' as plugin;

/// The device's microphone permission, through `permission_handler`.
///
/// Neither platform can tell "never asked" from "refused once" without asking: Android reports
/// both as denied until a request returns, and so does this. Callers therefore request whenever
/// the status is not [MicrophonePermissionStatus.granted] or
/// [MicrophonePermissionStatus.permanentlyDenied]. It never reports
/// [MicrophonePermissionStatus.notDetermined].
///
/// On Android, revoking the permission in system settings kills the app's process, so a running
/// app never sees it change underneath it; the next launch starts denied.
class PlatformMicrophonePermission implements MicrophonePermission {
  /// Creates the platform permission.
  const new();

  @override
  Future<MicrophonePermissionStatus> status() async =>
      fromPluginStatus(await plugin.Permission.microphone.status);

  @override
  Future<MicrophonePermissionStatus> request() async =>
      fromPluginStatus(await plugin.Permission.microphone.request());

  @override
  Future<bool> openSettings() => plugin.openAppSettings();
}

/// Maps the plugin's status onto ours. Parental controls ([plugin.PermissionStatus.restricted])
/// are as final as a permanent refusal from the app's point of view.
@visibleForTesting
MicrophonePermissionStatus fromPluginStatus(plugin.PermissionStatus status) => switch (status) {
  plugin.PermissionStatus.granted ||
  plugin.PermissionStatus.limited ||
  plugin.PermissionStatus.provisional => MicrophonePermissionStatus.granted,
  plugin.PermissionStatus.denied => MicrophonePermissionStatus.denied,
  plugin.PermissionStatus.permanentlyDenied ||
  plugin.PermissionStatus.restricted => MicrophonePermissionStatus.permanentlyDenied,
};
