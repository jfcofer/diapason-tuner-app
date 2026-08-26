import 'package:audio_engine/src/rust/api/simple.dart' as ffi;
import 'package:audio_engine/src/rust/frb_generated.dart';
import 'package:core_domain/core_domain.dart';

/// Talks to the Rust engine.
///
/// Implementations: [AudioEngine] over the real bridge, and `FakeAudioEngine` in memory. Features
/// and widget tests depend on this interface, never on the generated bindings, so that a UI test
/// never has to load a native library.
abstract interface class EngineHandle {
  /// Prepare the engine. Must complete before any other call.
  Future<void> initialize();

  /// Read the engine's current status.
  EngineStatus status();
}

/// The real engine, over the flutter_rust_bridge boundary.
class AudioEngine implements EngineHandle {
  /// Creates a handle. Call [initialize] before anything else.
  AudioEngine();

  bool _initialized = false;

  @override
  Future<void> initialize() async {
    if (_initialized) return;
    await RustLib.init();
    _initialized = true;
  }

  @override
  EngineStatus status() {
    assert(_initialized, 'call initialize() before status()');
    final status = ffi.engineStatus();
    // Map the generated type into the domain type here, at the boundary, so nothing above this
    // layer holds a bridge-generated object.
    return EngineStatus(
      dspBuild: status.dspBuild,
      engineBuild: status.engineBuild,
      running: status.running,
    );
  }
}
