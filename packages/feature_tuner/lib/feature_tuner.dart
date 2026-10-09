/// The tuner feature: screen, view models and tuner-only widgets.
///
/// `feature_*` packages never import each other (`AGENTS.md` §5). Anything two features need moves
/// **down** into `core_*`, never sideways.
///
/// `T-001` ships a screen that displays one value returned from Rust across the FFI boundary. The
/// strobe ring and everything else in `docs/DESIGN_SYSTEM.md` arrives once the audio spine works on
/// a real device - building it first would mean rebuilding it (`docs/BOOTSTRAP.md`).
library;

export 'src/tuner_screen.dart';
