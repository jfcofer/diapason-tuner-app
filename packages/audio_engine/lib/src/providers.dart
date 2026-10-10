import 'package:audio_engine/src/engine_facade.dart';
import 'package:core_domain/core_domain.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'providers.g.dart';

/// The engine in use: one per app, shared by every feature.
///
/// It lives here rather than in a feature because features never import each other
/// (`AGENTS.md` §5), and both the tuner and the metronome drive the same stream. Overridden in
/// `bootstrap.dart` with a real [AudioEngine], and in tests with a `FakeAudioEngine`. Throwing here
/// rather than defaulting to the real engine is deliberate: a widget test that forgets the override
/// should fail loudly, not quietly try to load a native library.
@Riverpod(keepAlive: true)
EngineHandle engineHandle(Ref ref) =>
    throw UnimplementedError('override engineHandleProvider in bootstrap.dart or in your test');

/// Which builds of the engine are running.
@riverpod
EngineStatus engineStatus(Ref ref) => ref.watch(engineHandleProvider).status();

/// The session, about 30 times a second.
///
/// Watch it with `select` for the discrete fields you need, so a widget rebuilds when its state
/// changes rather than on every snapshot. Continuous values such as the input level belong in a
/// painter fed straight from the stream (`docs/ARCHITECTURE.md` §5).
@Riverpod(keepAlive: true)
Stream<SessionSnapshot> sessionSnapshot(Ref ref) => ref.watch(engineHandleProvider).snapshots;
