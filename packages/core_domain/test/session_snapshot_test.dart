import 'package:core_domain/core_domain.dart';
import 'package:test/test.dart';

void main() {
  group('SessionSnapshot', () {
    const diagnostics = StreamDiagnostics(
      backend: 'aaudio',
      maxBlockFrames: 1024,
      outputPath: GrantedPath(lowLatency: true, exclusive: false),
    );
    const snapshot = SessionSnapshot(
      state: SessionState.running,
      sampleRate: 48000,
      diagnostics: diagnostics,
      inputActive: true,
    );

    test('compares by value, nested diagnostics included', () {
      expect(
        snapshot,
        equals(
          const SessionSnapshot(
            state: SessionState.running,
            sampleRate: 48000,
            diagnostics: StreamDiagnostics(
              backend: 'aaudio',
              maxBlockFrames: 1024,
              outputPath: GrantedPath(lowLatency: true, exclusive: false),
            ),
            inputActive: true,
          ),
        ),
      );
      expect(snapshot.hashCode, equals(snapshot.copyWith().hashCode));
      expect(
        snapshot,
        isNot(
          equals(
            snapshot.copyWith(
              diagnostics: const StreamDiagnostics(backend: 'aaudio', maxBlockFrames: 512),
            ),
          ),
        ),
      );
    });

    test('copyWith replaces only what it is given', () {
      final failed = snapshot.copyWith(
        state: SessionState.failed,
        fault: AudioFault.deviceUnavailable,
      );
      expect(failed.state, SessionState.failed);
      expect(failed.fault, AudioFault.deviceUnavailable);
      expect(failed.inputActive, isTrue);
      expect(failed.diagnostics, same(diagnostics));
    });
  });
  group('callbackLoad', () {
    SessionSnapshot running({int? burst, Duration worst = Duration.zero, int rate = 48000}) =>
        SessionSnapshot(
          state: SessionState.running,
          sampleRate: rate,
          diagnostics: StreamDiagnostics(
            backend: 'aaudio',
            framesPerBurst: burst,
            worstCallback: worst,
          ),
        );

    test('is the worst callback over one burst period', () {
      // 960 frames at 48 kHz is 20 ms; 3 ms of it is 15 %, the budget.
      final load = running(burst: 960, worst: const Duration(milliseconds: 3)).callbackLoad;
      expect(load, closeTo(0.15, 1e-9));
    });

    test('is unknown with no burst or no stream', () {
      expect(running().callbackLoad, isNull);
      expect(running(burst: 960, rate: 0).callbackLoad, isNull);
    });
  });
}
