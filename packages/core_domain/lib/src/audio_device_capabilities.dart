import 'package:meta/meta.dart';

/// What the platform reports about the audio device before any stream opens
/// (`docs/PLATFORM_AUDIO.md` §2). Every field is `null` where the platform reports nothing, and
/// unknown is never read as supported.
///
/// The engine chooses the microphone's input preset from [unprocessedSource] and [lowLatency]
/// (`docs/adr/0024`). The rest is for diagnostics: the native rate and buffer are what the device
/// prefers, to set beside what a stream was actually granted.
@immutable
class AudioDeviceCapabilities {
  /// Creates capabilities. Omitted fields are unknown.
  const new({
    this.unprocessedSource,
    this.lowLatency,
    this.proAudio,
    this.nativeSampleRate,
    this.nativeFramesPerBuffer,
  });

  /// Nothing known: a platform with no capabilities channel, or a channel that failed.
  static const unknown = AudioDeviceCapabilities();

  /// The microphone has an unprocessed source (Android:
  /// `PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED`).
  final bool? unprocessedSource;

  /// The device advertises a low-latency audio path (Android: `FEATURE_AUDIO_LOW_LATENCY`).
  final bool? lowLatency;

  /// The device advertises professional audio (Android: `FEATURE_AUDIO_PRO`).
  final bool? proAudio;

  /// The output rate the device prefers, in hertz.
  final int? nativeSampleRate;

  /// The output buffer the device prefers, in frames.
  final int? nativeFramesPerBuffer;

  @override
  bool operator ==(Object other) =>
      other is AudioDeviceCapabilities &&
      other.unprocessedSource == unprocessedSource &&
      other.lowLatency == lowLatency &&
      other.proAudio == proAudio &&
      other.nativeSampleRate == nativeSampleRate &&
      other.nativeFramesPerBuffer == nativeFramesPerBuffer;

  @override
  int get hashCode =>
      Object.hash(unprocessedSource, lowLatency, proAudio, nativeSampleRate, nativeFramesPerBuffer);

  @override
  String toString() =>
      'AudioDeviceCapabilities(unprocessed: $unprocessedSource, lowLatency: $lowLatency, '
      'pro: $proAudio, native: $nativeSampleRate Hz / $nativeFramesPerBuffer frames)';
}
