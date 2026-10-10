// The platform channel for audio capabilities (docs/adr/0024). Pigeon generates both ends from
// this file: `just gen-dart` runs it, and the output is not committed. Declarations only.
import 'package:pigeon/pigeon.dart';

@ConfigurePigeon(
  PigeonOptions(
    dartOut: 'lib/src/audio_capabilities_api.g.dart',
    dartOptions: DartOptions(),
    kotlinOut:
        'android/src/main/kotlin/dev/jfcofer/diapason/core_platform/AudioCapabilitiesApi.g.kt',
    kotlinOptions: KotlinOptions(package: 'dev.jfcofer.diapason.core_platform'),
    dartPackageName: 'core_platform',
  ),
)
/// What Android reports about the audio device. Null where it reports nothing.
class AudioCapabilitiesMessage {
  /// `AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED`.
  bool? unprocessedSource;

  /// `PackageManager.FEATURE_AUDIO_LOW_LATENCY`.
  bool? lowLatency;

  /// `PackageManager.FEATURE_AUDIO_PRO`.
  bool? proAudio;

  /// `AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE`.
  int? nativeSampleRate;

  /// `AudioManager.PROPERTY_OUTPUT_FRAMES_PER_BUFFER`.
  int? nativeFramesPerBuffer;
}

@HostApi()
abstract class AudioCapabilitiesApi {
  /// Read the capabilities. Cheap, and safe to call before any stream opens.
  AudioCapabilitiesMessage read();
}
