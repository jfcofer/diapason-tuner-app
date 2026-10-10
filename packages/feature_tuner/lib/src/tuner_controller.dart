import 'package:audio_engine/audio_engine.dart';
import 'package:core_platform/core_platform.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

part 'tuner_controller.g.dart';

/// The microphone permission. Only the tuner needs it, so it is provided here; `bootstrap.dart`
/// overrides it with `PlatformMicrophonePermission`, tests with a fake. Throws if not overridden,
/// for the same reason `engineHandleProvider` does.
@Riverpod(keepAlive: true)
MicrophonePermission microphonePermission(Ref ref) => throw UnimplementedError(
  'override microphonePermissionProvider in bootstrap.dart or your test',
);

/// Whether development diagnostics show: the A4 test tone and the stream's preset readout. Off
/// unless overridden, so a build that forgets the override ships without them; `bootstrap.dart`
/// overrides it from the flavour (`docs/CI_RELEASE.md` §6).
@Riverpod(keepAlive: true)
bool showEngineDiagnostics(Ref ref) => false;

/// The tuner's control flow: permission first, then the stream (`docs/PLATFORM_AUDIO.md` §2).
///
/// State is the microphone permission as of the last attempt to listen, or `null` before the
/// first one, which is when the screen explains why it wants the microphone. Whether the
/// microphone is actually open is the engine's to say, in the session snapshot.
@riverpod
class TunerController extends _$TunerController {
  @override
  MicrophonePermissionStatus? build() => null;

  /// Ask for the microphone if needed, tell the engine the answer, then open the stream with the
  /// microphone if granted. Without it, no stream is opened for the tuner's sake: the screen shows
  /// how to recover instead.
  Future<void> listen() async {
    final permission = ref.read(microphonePermissionProvider);
    var status = await permission.status();
    if (status != MicrophonePermissionStatus.granted &&
        status != MicrophonePermissionStatus.permanentlyDenied) {
      status = await permission.request();
    }
    if (!ref.mounted) return;
    state = status;
    final granted = status == MicrophonePermissionStatus.granted;
    // The engine cannot tell a refused microphone from a broken one by itself (docs/adr/0024).
    final engine = ref.read(engineHandleProvider)..setMicrophoneAccess(granted: granted);
    if (granted) engine.start(input: true);
  }

  /// Ask for the microphone again after the engine reported an input fault, with the permission
  /// already granted: a device that was busy or briefly gone may be back.
  void retryMicrophone() => ref.read(engineHandleProvider).start(input: true);

  /// Open the system settings, the only way back from a permanent refusal.
  Future<void> openSettings() => ref.read(microphonePermissionProvider).openSettings();

  /// Development aid until the tuner UI lands (M3): a 440 Hz tone proves the output path, and it
  /// works with the microphone refused, as the metronome must. Opens an output-only stream if none
  /// is running.
  void toggleTestTone({required bool playing, required bool streamRunning}) {
    final engine = ref.read(engineHandleProvider);
    if (playing) {
      engine.stopTone();
      return;
    }
    if (!streamRunning) engine.start(input: false);
    engine.startTone(frequencyHz: 440, amplitude: 0.2);
  }
}
