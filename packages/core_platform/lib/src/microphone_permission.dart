/// Whether the app may open the microphone.
enum MicrophonePermissionStatus {
  /// The user has not been asked yet.
  notDetermined,

  /// Granted; a capture stream may be opened.
  granted,

  /// Refused, and the OS will show the prompt again if asked.
  denied,

  /// Refused permanently. Only a trip to system settings can change this.
  permanentlyDenied,
}

/// Reads and requests the microphone permission.
///
/// Implementations: `PlatformMicrophonePermission` on a device, [FakeMicrophonePermission] in
/// tests. The microphone is requested only when the user first opens the tuner, after an
/// explanation (`docs/PLATFORM_AUDIO.md` §2).
abstract interface class MicrophonePermission {
  /// The current status, without prompting.
  Future<MicrophonePermissionStatus> status();

  /// Prompt if possible, then return the resulting status.
  Future<MicrophonePermissionStatus> request();

  /// Open this app's page in the system settings: the only way back from
  /// [MicrophonePermissionStatus.permanentlyDenied]. Returns whether the page could be opened.
  Future<bool> openSettings();
}

/// In-memory [MicrophonePermission] for tests.
///
/// Every implementation in this package ships one of these; a widget test that needs a permission
/// should never reach a real platform channel.
class FakeMicrophonePermission implements MicrophonePermission {
  /// Creates a fake that reports [initial] and, when asked, moves to [onRequest].
  new({
    MicrophonePermissionStatus initial = MicrophonePermissionStatus.notDetermined,
    this.onRequest = MicrophonePermissionStatus.granted,
  }) : _status = initial;

  MicrophonePermissionStatus _status;

  /// The status [request] will settle on.
  final MicrophonePermissionStatus onRequest;

  /// How many times [request] has been called.
  int requestCount = 0;

  /// How many times [openSettings] has been called.
  int settingsOpened = 0;

  @override
  Future<MicrophonePermissionStatus> status() async => _status;

  @override
  Future<MicrophonePermissionStatus> request() async {
    requestCount++;
    return _status = onRequest;
  }

  @override
  Future<bool> openSettings() async {
    settingsOpened++;
    return true;
  }
}
