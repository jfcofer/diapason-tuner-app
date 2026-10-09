import 'package:meta/meta.dart';

/// What the Rust engine reports about itself.
///
/// Deliberately a plain immutable value rather than the generated FFI type: the generated type is
/// an implementation detail of `audio_engine`, and every layer above this one should be able to
/// hold engine state without depending on the bridge.
@immutable
class EngineStatus {
  /// Creates a status snapshot.
  const new({required this.dspBuild, required this.engineBuild, required this.running});

  /// Identifies the DSP crate at the bottom of the Rust stack.
  final String dspBuild;

  /// Identifies the engine crate.
  final String engineBuild;

  /// Whether an audio stream is currently running.
  final bool running;

  /// Returns a copy with the given fields replaced.
  EngineStatus copyWith({String? dspBuild, String? engineBuild, bool? running}) => EngineStatus(
    dspBuild: dspBuild ?? this.dspBuild,
    engineBuild: engineBuild ?? this.engineBuild,
    running: running ?? this.running,
  );

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EngineStatus &&
          other.dspBuild == dspBuild &&
          other.engineBuild == engineBuild &&
          other.running == running;

  @override
  int get hashCode => Object.hash(dspBuild, engineBuild, running);

  @override
  String toString() => 'EngineStatus($dspBuild, $engineBuild, running: $running)';
}
