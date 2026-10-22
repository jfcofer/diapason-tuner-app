import 'dart:async';

import 'package:audio_engine/audio_engine.dart';
import 'package:core_platform/core_platform.dart';
import 'package:diapason/app.dart';
import 'package:diapason/flavor.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Starts the app with the engine initialised and the platform providers overridden.
///
/// Every flavour entrypoint funnels through here so that initialisation order is defined in exactly
/// one place. The engine is initialised *before* `runApp` because `engineStatusProvider` reads it
/// synchronously during the first build. It needs the device's audio capabilities first, from
/// which it chooses the microphone's input preset (`docs/adr/0024`). Initialising it starts the
/// session thread; no stream opens, and no permission is asked for, until the user chooses to.
Future<void> bootstrap() async {
  WidgetsFlutterBinding.ensureInitialized();

  FlutterError.onError = (details) {
    FlutterError.presentError(details);
    if (kReleaseMode) {
      // No telemetry (docs/adr/0010). In release the error is presented and nothing leaves the
      // device; opt-in crash reporting, if it ever ships, hooks in here and nowhere else.
    }
  };

  runApp(await diapasonRoot());
}

/// The app with every platform provider overridden: what [bootstrap] runs, and what the device
/// tests pump, so the two cannot drift apart. They pass their own [engine] to watch its sessions;
/// otherwise one is made for this device's audio capabilities.
Future<Widget> diapasonRoot({EngineHandle? engine}) async {
  final handle = engine ?? AudioEngine(capabilities: await PlatformAudioCapabilities().read());
  await handle.initialize();
  return ProviderScope(
    overrides: [
      engineHandleProvider.overrideWithValue(handle),
      microphonePermissionProvider.overrideWithValue(const PlatformMicrophonePermission()),
      appLifecycleProvider.overrideWithValue(FlutterAppLifecycle()),
      showEngineDiagnosticsProvider.overrideWithValue(Flavor.showEngineDiagnostics),
    ],
    child: const DiapasonApp(),
  );
}
