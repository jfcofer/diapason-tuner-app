import 'dart:async';

import 'package:audio_engine/audio_engine.dart';
import 'package:core_platform/core_platform.dart';
import 'package:feature_tuner/src/listening_policy.dart';
import 'package:flutter/widgets.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'tuner_controller.g.dart';

/// The microphone permission. Only the tuner needs it, so it is provided here; `bootstrap.dart`
/// overrides it with `PlatformMicrophonePermission`, tests with a fake. Throws if not overridden,
/// for the same reason `engineHandleProvider` does.
@Riverpod(keepAlive: true)
MicrophonePermission microphonePermission(Ref ref) => throw UnimplementedError(
  'override microphonePermissionProvider in bootstrap.dart or your test',
);

/// The app's lifecycle. `bootstrap.dart` overrides it with `FlutterAppLifecycle`, tests with
/// `FakeAppLifecycle`. Throws if not overridden, for the same reason `engineHandleProvider` does.
@Riverpod(keepAlive: true)
AppLifecycle appLifecycle(Ref ref) =>
    throw UnimplementedError('override appLifecycleProvider in bootstrap.dart or your test');

/// Whether development diagnostics show: the A4 test tone and the stream's preset readout. Off
/// unless overridden, so a build that forgets the override ships without them; `bootstrap.dart`
/// overrides it from the flavour (`docs/CI_RELEASE.md` §6).
@Riverpod(keepAlive: true)
bool showEngineDiagnostics(Ref ref) => false;

/// What the tuner knows and what the user asked of it.
///
/// Whether the microphone is actually open is the engine's to say, in the session snapshot.
@immutable
final class TunerState {
  /// Creates a state; the default is the tuner before the user has asked for anything.
  const new({this.permission, this.listenWanted = false, this.toneWanted = false});

  /// The microphone permission as last read, or `null` while the screen should still explain why
  /// it wants the microphone: Android cannot tell "never asked" from "refused once" without asking.
  final MicrophonePermissionStatus? permission;

  /// The user asked to listen, and has neither stopped nor left the tuner since. It survives a
  /// refusal, so a grant made in Settings starts listening on the way back.
  final bool listenWanted;

  /// The user started the development A4 tone.
  final bool toneWanted;

  /// A copy with the given fields replaced.
  TunerState copyWith({
    MicrophonePermissionStatus? permission,
    bool? listenWanted,
    bool? toneWanted,
  }) => TunerState(
    permission: permission ?? this.permission,
    listenWanted: listenWanted ?? this.listenWanted,
    toneWanted: toneWanted ?? this.toneWanted,
  );

  @override
  bool operator ==(Object other) =>
      other is TunerState &&
      other.permission == permission &&
      other.listenWanted == listenWanted &&
      other.toneWanted == toneWanted;

  @override
  int get hashCode => Object.hash(permission, listenWanted, toneWanted);
}

/// The tuner's control flow: permission first, then the stream (`docs/PLATFORM_AUDIO.md` §2), and
/// the stream only while the tuner is in front of the user (§5).
///
/// It owns what the user asked for, and after every change hands [engineTarget] that, the
/// permission, the app's lifecycle and the screen's visibility, then moves the engine to the
/// target. Disposing it (leaving the tuner) stops everything it started.
@riverpod
class TunerController extends _$TunerController {
  late EngineHandle _engine;
  var _foreground = true;
  var _visible = true;
  EngineTarget _applied = EngineTarget.stopped;
  var _toneSent = false;

  @override
  TunerState build() {
    // Captured now: `ref` cannot be used once disposal has begun, which is when it is needed most.
    _engine = ref.read(engineHandleProvider);
    final lifecycle = ref.read(appLifecycleProvider);
    _foreground = _isForeground(lifecycle.current);
    final subscription = lifecycle.changes.listen(_onLifecycle);
    ref.onDispose(() {
      unawaited(subscription.cancel());
      _release();
    });
    unawaited(_readPermission());
    return const TunerState();
  }

  /// Ask for the microphone if needed, tell the engine the answer, then listen if granted. Without
  /// it, no stream is opened for the tuner's sake: the screen shows how to recover instead.
  Future<void> listen() async {
    final permission = ref.read(microphonePermissionProvider);
    var status = await permission.status();
    if (status != MicrophonePermissionStatus.granted &&
        status != MicrophonePermissionStatus.permanentlyDenied) {
      status = await permission.request();
    }
    if (!ref.mounted) return;
    // The engine cannot tell a refused microphone from a broken one by itself (docs/adr/0024).
    _engine.setMicrophoneAccess(granted: status == MicrophonePermissionStatus.granted);
    state = state.copyWith(permission: status, listenWanted: true);
    _apply();
  }

  /// Stop listening and release the microphone.
  void stopListening() {
    state = state.copyWith(listenWanted: false);
    _apply();
  }

  /// Ask for the microphone again after the engine reported an input fault, with the permission
  /// already granted: a device that was busy or briefly gone may be back.
  void retryMicrophone() {
    if (_applied == EngineTarget.duplex) _engine.start(input: true);
  }

  /// Open the system settings, the only way back from a permanent refusal.
  Future<void> openSettings() => ref.read(microphonePermissionProvider).openSettings();

  /// Development aid until the tuner UI lands (M3): a 440 Hz tone proves the output path, and it
  /// works with the microphone refused, as the metronome must.
  void toggleTestTone() {
    state = state.copyWith(toneWanted: !state.toneWanted);
    _apply();
  }

  /// Whether the tuner is on screen, as `TunerVisibility` sees it. Leaving it ends what the user
  /// asked for, so coming back never starts the microphone unasked.
  void setVisible({required bool visible}) {
    if (visible == _visible) return;
    _visible = visible;
    if (!visible) state = state.copyWith(listenWanted: false, toneWanted: false);
    _apply();
  }

  /// The permission when the tuner opens, read without prompting. Only a settled answer is kept:
  /// "denied" may mean "never asked", which must still be explained first.
  Future<void> _readPermission() async {
    final status = await ref.read(microphonePermissionProvider).status();
    // A tap on the screen may have answered first, with fresher news.
    if (!ref.mounted || state.listenWanted) return;
    _engine.setMicrophoneAccess(granted: status == MicrophonePermissionStatus.granted);
    state = state.copyWith(permission: _settled(status, state.permission));
  }

  void _onLifecycle(AppLifecycleState lifecycle) {
    final foreground = _isForeground(lifecycle);
    // From inactive to resumed is the permission dialog or the notification shade closing.
    if (foreground == _foreground) return;
    _foreground = foreground;
    if (foreground) {
      unawaited(_returnToForeground());
    } else {
      _apply();
    }
  }

  /// Back in front: the permission may have changed in Settings meanwhile, so read it before
  /// resuming (`docs/PLATFORM_AUDIO.md` §5).
  Future<void> _returnToForeground() async {
    final status = await ref.read(microphonePermissionProvider).status();
    if (!ref.mounted || !_foreground) return;
    _engine.setMicrophoneAccess(granted: status == MicrophonePermissionStatus.granted);
    state = state.copyWith(permission: _settled(status, state.permission));
    _apply();
  }

  /// Move the engine to the policy's target, sending only what changed.
  void _apply() {
    if (state.toneWanted != _toneSent) {
      _toneSent = state.toneWanted;
      if (_toneSent) {
        _engine.startTone(frequencyHz: 440, amplitude: 0.2);
      } else {
        _engine.stopTone();
      }
    }
    final target = engineTarget(
      listenWanted: state.listenWanted,
      toneWanted: state.toneWanted,
      granted: state.permission == MicrophonePermissionStatus.granted,
      foreground: _foreground,
      visible: _visible,
    );
    if (target == _applied) return;
    _applied = target;
    switch (target) {
      case EngineTarget.stopped:
        _engine.stop();
      case EngineTarget.outputOnly:
        _engine.start(input: false);
      case EngineTarget.duplex:
        _engine.start(input: true);
    }
  }

  /// Leaving the tuner. The session keeps a tone across a stop and replays it on the next start
  /// (`docs/adr/0022`), so the tone is stopped first: the metronome must not open with an A4.
  void _release() {
    if (_toneSent) _engine.stopTone();
    if (_applied != EngineTarget.stopped) _engine.stop();
  }
}

bool _isForeground(AppLifecycleState state) => switch (state) {
  AppLifecycleState.resumed || AppLifecycleState.inactive => true,
  AppLifecycleState.hidden || AppLifecycleState.paused || AppLifecycleState.detached => false,
};

/// The status worth showing: a settled answer, or a refusal once the user has been asked.
MicrophonePermissionStatus? _settled(
  MicrophonePermissionStatus status,
  MicrophonePermissionStatus? previous,
) => switch (status) {
  MicrophonePermissionStatus.granted || MicrophonePermissionStatus.permanentlyDenied => status,
  _ when previous == null => null,
  _ => status,
};
