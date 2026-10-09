import 'package:core_domain/core_domain.dart';
import 'package:test/test.dart';

void main() {
  group('EngineStatus', () {
    const status = EngineStatus(dspBuild: 'dsp 0.1.0', engineBuild: 'engine 0.1.0');

    test('compares by value, so it can drive rebuilds without identity churn', () {
      expect(
        status,
        equals(const EngineStatus(dspBuild: 'dsp 0.1.0', engineBuild: 'engine 0.1.0')),
      );
    });

    test('copyWith replaces only what it is given', () {
      expect(status.copyWith(engineBuild: 'engine 0.2.0').engineBuild, equals('engine 0.2.0'));
      expect(status.copyWith(engineBuild: 'engine 0.2.0').dspBuild, equals(status.dspBuild));
    });
  });
}
