import 'package:core_ui/core_ui.dart';
import 'package:diapason/flavor.dart';
import 'package:diapason/router.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

/// The application shell: theme wiring, localisation delegates and the router.
///
/// This widget owns no behaviour. `apps/diapason` is the shell and nothing else
/// (`docs/REPO_LAYOUT.md`); anything that looks like a feature belongs in a `feature_*` package.
class DiapasonApp extends StatelessWidget {
  /// Creates the app.
  const DiapasonApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp.router(
    title: Flavor.appTitle,
    debugShowCheckedModeBanner: Flavor.current != Flavor.prod,
    theme: DiapasonTheme.light(),
    darkTheme: DiapasonTheme.dark(),
    routerConfig: appRouter,
    localizationsDelegates: const <LocalizationsDelegate<Object>>[
      GlobalMaterialLocalizations.delegate,
      GlobalWidgetsLocalizations.delegate,
      GlobalCupertinoLocalizations.delegate,
    ],
    // English and Spanish from the start: the app is named in Spanish and the store listing is
    // bilingual (docs/CI_RELEASE.md §5). Content strings arrive with the UI; T-001 ships the
    // plumbing only, because the locale-aware app *label* depends on it.
    supportedLocales: const <Locale>[Locale('en'), Locale('es')],
  );
}
