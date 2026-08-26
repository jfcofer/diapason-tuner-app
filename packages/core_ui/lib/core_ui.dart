/// The design system: tokens, theme, primitives, painters and motion constants.
///
/// **No Riverpod and no plugins** (`AGENTS.md` §5). Widgets here take their data as arguments, so
/// they are trivially golden-testable and reusable from any feature.
///
/// `T-001` ships theme wiring only - the real tokens, the strobe ring and the rest of
/// `docs/DESIGN_SYSTEM.md` arrive at milestone M3.
library;

export 'src/theme.dart';
