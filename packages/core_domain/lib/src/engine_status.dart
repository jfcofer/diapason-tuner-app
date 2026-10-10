import 'package:meta/meta.dart';

/// Which builds of the Rust engine are running, for the about screen and bug reports.
///
/// Deliberately a plain immutable value rather than the generated FFI type: the generated type is
/// an implementation detail of `audio_engine`, and every layer above this one should be able to
/// hold engine state without depending on the bridge. Stream state is a `SessionSnapshot` instead.
@immutable
class EngineStatus {
  /// Creates a status snapshot.
  const new({required this.dspBuild, required this.engineBuild});

  /// Identifies the DSP crate at the bottom of the Rust stack.
  final String dspBuild;

  /// Identifies the engine crate.
  final String engineBuild;

  /// Returns a copy with the given fields replaced.
  EngineStatus copyWith({String? dspBuild, String? engineBuild}) => EngineStatus(
    dspBuild: dspBuild ?? this.dspBuild,
    engineBuild: engineBuild ?? this.engineBuild,
  );

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is EngineStatus && other.dspBuild == dspBuild && other.engineBuild == engineBuild;

  @override
  int get hashCode => Object.hash(dspBuild, engineBuild);

  @override
  String toString() => 'EngineStatus($dspBuild, $engineBuild)';
}
