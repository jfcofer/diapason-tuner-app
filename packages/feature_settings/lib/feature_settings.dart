/// The settings feature: screen, view models and settings-only widgets.
///
/// `feature_*` packages never import each other (`AGENTS.md` §5). Anything two features need
/// moves **down** into `core_*`, never sideways.
///
/// `T-001` ships a placeholder screen so the package, its barrel and its test are real and the
/// dependency-direction check has something to enforce. The feature itself arrives at its own
/// milestone.
library;

export 'src/settings_screen.dart';
