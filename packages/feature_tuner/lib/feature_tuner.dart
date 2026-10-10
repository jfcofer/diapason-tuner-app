/// The tuner feature: screen, view models and tuner-only widgets.
///
/// `feature_*` packages never import each other (`AGENTS.md` §5). Anything two features need moves
/// **down** into `core_*`, never sideways.
///
/// Until M3 the screen is audio-spine scaffolding: build info, the microphone permission flow and
/// the live session. The strobe ring and everything else in `docs/DESIGN_SYSTEM.md` arrives once
/// the spine works on a real device - building it first would mean rebuilding it.
library;

export 'src/tuner_controller.dart';
export 'src/tuner_screen.dart';
