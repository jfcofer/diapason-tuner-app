/// The Dart face of the Rust audio engine.
///
/// Dart never touches an audio buffer (`docs/adr/0001`). It sends commands and receives snapshots.
/// Everything in `src/rust/` is generated - regenerate it with `just gen`, never hand-edit it
/// (`docs/REPO_LAYOUT.md`).
///
/// Depend on `AudioEngine` rather than on the generated bindings: the facade is what keeps the
/// generated types from leaking into features, and it is what `FakeAudioEngine` can stand in for.
library;

export 'src/engine_facade.dart';
export 'src/fake_engine.dart';
