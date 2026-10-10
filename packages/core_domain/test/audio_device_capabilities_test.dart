import 'package:core_domain/core_domain.dart';
import 'package:test/test.dart';

void main() {
  test('unknown knows nothing', () {
    const unknown = AudioDeviceCapabilities.unknown;
    expect(unknown.unprocessedSource, isNull);
    expect(unknown.lowLatency, isNull);
    expect(unknown.nativeSampleRate, isNull);
  });

  test('compares by value', () {
    expect(
      const AudioDeviceCapabilities(lowLatency: true, nativeSampleRate: 48000),
      equals(const AudioDeviceCapabilities(lowLatency: true, nativeSampleRate: 48000)),
    );
    expect(
      const AudioDeviceCapabilities(lowLatency: true),
      isNot(equals(const AudioDeviceCapabilities(lowLatency: false))),
    );
  });
}
