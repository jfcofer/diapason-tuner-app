import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:core_platform/src/audio_capabilities_api.g.dart';
import 'package:core_platform/src/platform_audio_capabilities.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// The generated API with its channel replaced by an answer or a failure.
class _Api extends AudioCapabilitiesApi {
  new(this._answer);

  final Future<AudioCapabilitiesMessage> Function() _answer;

  @override
  Future<AudioCapabilitiesMessage> read() => _answer();
}

void main() {
  final message = AudioCapabilitiesMessage(
    unprocessedSource: true,
    lowLatency: true,
    proAudio: false,
    nativeSampleRate: 48000,
    nativeFramesPerBuffer: 192,
  );

  test('the fake reports what it was given and counts reads', () async {
    final fake = FakeAudioCapabilities(const AudioDeviceCapabilities(lowLatency: true));
    expect(await fake.read(), const AudioDeviceCapabilities(lowLatency: true));
    expect(fake.readCount, 1);
  });

  test('every field of the message reaches the domain type', () {
    expect(
      fromMessage(message),
      const AudioDeviceCapabilities(
        unprocessedSource: true,
        lowLatency: true,
        proAudio: false,
        nativeSampleRate: 48000,
        nativeFramesPerBuffer: 192,
      ),
    );
  });

  group('on Android', () {
    setUp(() => debugDefaultTargetPlatformOverride = TargetPlatform.android);
    tearDown(() => debugDefaultTargetPlatformOverride = null);

    test('reads the channel', () async {
      final reader = PlatformAudioCapabilities(api: _Api(() async => message));
      expect((await reader.read()).nativeFramesPerBuffer, 192);
    });

    test('a channel that fails reports unknown rather than throwing', () async {
      final failing = PlatformAudioCapabilities(
        api: _Api(() => Future.error(PlatformException(code: 'channel-error'))),
      );
      expect(await failing.read(), AudioDeviceCapabilities.unknown);
      final missing = PlatformAudioCapabilities(
        api: _Api(() => Future.error(MissingPluginException())),
      );
      expect(await missing.read(), AudioDeviceCapabilities.unknown);
    });
  });

  test('anywhere else, unknown without touching the channel', () async {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
    var asked = false;
    final reader = PlatformAudioCapabilities(
      api: _Api(() async {
        asked = true;
        return message;
      }),
    );
    expect(await reader.read(), AudioDeviceCapabilities.unknown);
    expect(asked, isFalse);
  });
}
