/// Which build this is.
///
/// Comes from `--dart-define-from-file=flavors/<f>.json`. There is no `.env` and no runtime
/// configuration file (`docs/CI_RELEASE.md` §2), so this is resolved at compile time and is
/// tree-shakeable.
enum Flavor {
  /// Local development. Diagnostics on.
  dev,

  /// Internal testers: a release build with diagnostics still available.
  stg,

  /// The store build.
  prod;

  /// The flavour this binary was compiled for.
  ///
  /// `final`, not `const`: the switch below is not a constant expression. The *inputs* are all
  /// `String.fromEnvironment`, so this still resolves at startup from compile-time values.
  static final Flavor current = _fromName(
    const String.fromEnvironment('FLAVOR', defaultValue: 'dev'),
  );

  /// The title shown in the task switcher.
  static const String appTitle = String.fromEnvironment(
    'APP_TITLE',
    defaultValue: 'Diapason (Dev)',
  );

  /// Whether to expose the engine diagnostics overlay (`docs/CI_RELEASE.md` §6).
  static const bool showEngineDiagnostics = bool.fromEnvironment(
    'SHOW_ENGINE_DIAGNOSTICS',
    defaultValue: true,
  );

  static Flavor _fromName(String name) => switch (name) {
    'prod' => Flavor.prod,
    'stg' => Flavor.stg,
    _ => Flavor.dev,
  };
}
