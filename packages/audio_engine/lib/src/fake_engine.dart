import 'package:audio_engine/src/engine_facade.dart';
import 'package:core_domain/core_domain.dart';

/// In-memory [EngineHandle] used by every UI test.
///
/// Exists so that a widget test never loads a native library: goldens and widget tests must run in
/// the pinned container with no Rust build in sight (`docs/TESTING.md`).
class FakeAudioEngine implements EngineHandle {
  /// Creates a fake reporting [status], or a stopped placeholder engine.
  new({EngineStatus? status})
    : _status =
          status ??
          const EngineStatus(
            dspBuild: 'diapason_dsp 0.1.0 (fake)',
            engineBuild: 'diapason_engine 0.1.0 (fake)',
            running: false,
          );

  final EngineStatus _status;

  /// How many times [initialize] has been called.
  int initializeCount = 0;

  @override
  Future<void> initialize() async => initializeCount++;

  @override
  EngineStatus status() => _status;
}
