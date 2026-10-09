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
}
