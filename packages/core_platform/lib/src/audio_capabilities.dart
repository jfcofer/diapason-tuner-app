import 'package:core_domain/core_domain.dart';

/// Reads what the platform reports about the audio device, before any stream opens.
///
/// Implementations: `PlatformAudioCapabilities` on a device, [FakeAudioCapabilities] in tests. The
/// app reads it once at start-up and hands it to the engine, which chooses the microphone's input
/// preset from it (`docs/adr/0024`).
abstract interface class AudioCapabilities {
  /// The capabilities. Never throws: a platform that cannot answer reports
  /// [AudioDeviceCapabilities.unknown].
  Future<AudioDeviceCapabilities> read();
}

/// In-memory [AudioCapabilities] for tests.
class FakeAudioCapabilities implements AudioCapabilities {
  /// Creates a fake that reports [capabilities].
  new([this.capabilities = AudioDeviceCapabilities.unknown]);

  /// What [read] returns.
  final AudioDeviceCapabilities capabilities;

  /// How many times [read] has been called.
  int readCount = 0;

  @override
  Future<AudioDeviceCapabilities> read() async {
    readCount++;
    return capabilities;
  }
}
