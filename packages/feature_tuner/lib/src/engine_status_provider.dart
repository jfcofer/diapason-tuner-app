import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'engine_status_provider.g.dart';

/// The engine handle in use.
///
/// Overridden in `bootstrap.dart` with a real [AudioEngine], and in tests with a
/// [FakeAudioEngine]. Throwing here rather than defaulting to the real engine is deliberate: a
/// widget test that forgets the override should fail loudly, not quietly try to load a native
/// library.
@Riverpod(keepAlive: true)
EngineHandle engineHandle(Ref ref) =>
    throw UnimplementedError('override engineHandleProvider in bootstrap.dart or in your test');

/// The engine's current status.
///
/// Riverpod 3, code-generated only - no manual `Provider` globals anywhere in this repo
/// (`docs/adr/0004`). This is a one-shot read in `T-001`; it becomes a ~30 Hz snapshot stream once
/// there is an engine producing them.
@riverpod
EngineStatus engineStatus(Ref ref) => ref.watch(engineHandleProvider).status();
