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
/// `T-001` ships the interface and its fake only. The real implementation arrives with `T-002`,
/// when there is a stream that needs it; wiring a plugin before then would put a permission dialog
/// in front of an app that does not yet record anything.
abstract interface class MicrophonePermission {
  /// The current status, without prompting.
  Future<MicrophonePermissionStatus> status();

  /// Prompt if possible, then return the resulting status.
  Future<MicrophonePermissionStatus> request();
}

/// In-memory [MicrophonePermission] for tests and for the UI before `T-002` lands.
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

  @override
  Future<MicrophonePermissionStatus> status() async => _status;

  @override
  Future<MicrophonePermissionStatus> request() async {
    requestCount++;
    return _status = onRequest;
  }
}
