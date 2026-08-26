import 'package:feature_metronome/feature_metronome.dart';
import 'package:feature_settings/feature_settings.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:go_router/go_router.dart';

/// The app's routes.
///
/// `go_router` with typed routes (`AGENTS.md` §7). `T-001` wires the three destinations so the
/// dependency direction from the shell down into `feature_*` is real and enforced by
/// `just check-deps`; the shell chrome that lets a user move between them arrives with the UI.
final GoRouter appRouter = GoRouter(
  initialLocation: TunerRoute.path,
  routes: <RouteBase>[
    GoRoute(
      path: TunerRoute.path,
      name: TunerRoute.name,
      builder: (context, state) => const TunerScreen(),
    ),
    GoRoute(
      path: MetronomeRoute.path,
      name: MetronomeRoute.name,
      builder: (context, state) => const MetronomeScreen(),
    ),
    GoRoute(
      path: SettingsRoute.path,
      name: SettingsRoute.name,
      builder: (context, state) => const SettingsScreen(),
    ),
  ],
);

/// The tuner destination.
abstract final class TunerRoute {
  /// Route path.
  static const String path = '/';

  /// Route name, for `goNamed`.
  static const String name = 'tuner';
}

/// The metronome destination.
abstract final class MetronomeRoute {
  /// Route path.
  static const String path = '/metronome';

  /// Route name, for `goNamed`.
  static const String name = 'metronome';
}

/// The settings destination.
abstract final class SettingsRoute {
  /// Route path.
  static const String path = '/settings';

  /// Route name, for `goNamed`.
  static const String name = 'settings';
}
