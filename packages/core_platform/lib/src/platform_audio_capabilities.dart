import 'package:core_domain/core_domain.dart';
import 'package:core_platform/src/audio_capabilities.dart';
import 'package:core_platform/src/audio_capabilities_api.g.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// The device's audio capabilities, through this package's Pigeon channel (`docs/adr/0024`).
///
/// Android only for now: iOS reports [AudioDeviceCapabilities.unknown] until `T-002c`. Unknown is
/// safe, because the engine never reads it as supported: it falls back to the voice-recognition
/// preset.
class PlatformAudioCapabilities implements AudioCapabilities {
  /// Creates the platform reader. [api] is for tests.
  new({@visibleForTesting AudioCapabilitiesApi? api}) : _api = api ?? AudioCapabilitiesApi();

  final AudioCapabilitiesApi _api;

  @override
  Future<AudioDeviceCapabilities> read() async {
    if (defaultTargetPlatform != TargetPlatform.android) return AudioDeviceCapabilities.unknown;
    try {
      return fromMessage(await _api.read());
    } on PlatformException {
      return AudioDeviceCapabilities.unknown;
    } on MissingPluginException {
      return AudioDeviceCapabilities.unknown;
    }
  }
}

/// Maps the channel's message onto the domain type.
@visibleForTesting
AudioDeviceCapabilities fromMessage(AudioCapabilitiesMessage message) => AudioDeviceCapabilities(
  unprocessedSource: message.unprocessedSource,
  lowLatency: message.lowLatency,
  proAudio: message.proAudio,
  nativeSampleRate: message.nativeSampleRate,
  nativeFramesPerBuffer: message.nativeFramesPerBuffer,
);
