import 'dart:async';

import 'package:audio_engine/audio_engine.dart';
import 'package:diapason/app.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Starts the app inside an error-handling zone, with the engine initialised and its provider
/// overridden.
///
/// Every flavour entrypoint funnels through here so that initialisation order is defined in exactly
/// one place. The engine is initialised *before* `runApp` because `engineStatusProvider` reads it
/// synchronously during the first build.
Future<void> bootstrap() async {
  WidgetsFlutterBinding.ensureInitialized();

  FlutterError.onError = (details) {
    FlutterError.presentError(details);
    if (kReleaseMode) {
      // No telemetry (docs/adr/0010). In release the error is presented and nothing leaves the
      // device; opt-in crash reporting, if it ever ships, hooks in here and nowhere else.
    }
  };

  final engine = AudioEngine();
  await engine.initialize();

  runApp(
    ProviderScope(
      overrides: [engineHandleProvider.overrideWithValue(engine)],
      child: const DiapasonApp(),
    ),
  );
}
