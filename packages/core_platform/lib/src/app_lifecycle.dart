import 'dart:async';

import 'package:flutter/widgets.dart';

/// Where the app is in its lifecycle: in front of the user, or not.
///
/// Implementations: [FlutterAppLifecycle] on a device, [FakeAppLifecycle] in tests. Whatever holds
/// the microphone or a stream reacts to it (`docs/PLATFORM_AUDIO.md` §5): the tuner stops in the
/// background, and the metronome (M4) keeps playing.
abstract interface class AppLifecycle {
  /// The state now. [AppLifecycleState.resumed] before the platform has reported one.
  AppLifecycleState get current;

  /// Every change, in order, as it happens. Broadcast.
  Stream<AppLifecycleState> get changes;
}

/// The app's lifecycle as the Flutter binding reports it, through an [AppLifecycleListener].
///
/// One per app, for the life of the app: [dispose] exists for tests and for symmetry.
class FlutterAppLifecycle implements AppLifecycle {
  /// Starts listening to the binding, which must already be initialised.
  new() {
    _listener = AppLifecycleListener(onStateChange: _changes.add);
  }

  late final AppLifecycleListener _listener;
  final StreamController<AppLifecycleState> _changes = StreamController.broadcast();

  @override
  AppLifecycleState get current =>
      WidgetsBinding.instance.lifecycleState ?? AppLifecycleState.resumed;

  @override
  Stream<AppLifecycleState> get changes => _changes.stream;

  /// Stop listening to the binding.
  void dispose() {
    _listener.dispose();
    unawaited(_changes.close());
  }
}

/// In-memory [AppLifecycle] for tests: [set] plays the change the platform would report.
class FakeAppLifecycle implements AppLifecycle {
  /// Creates a fake that starts at [initial].
  new({AppLifecycleState initial = AppLifecycleState.resumed}) : _current = initial;

  AppLifecycleState _current;
  final StreamController<AppLifecycleState> _changes = StreamController.broadcast(sync: true);

  @override
  AppLifecycleState get current => _current;

  @override
  Stream<AppLifecycleState> get changes => _changes.stream;

  /// Move to [state] and report it, at once, to every listener.
  void set(AppLifecycleState state) {
    _current = state;
    _changes.add(state);
  }
}
