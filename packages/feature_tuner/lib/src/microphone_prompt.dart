import 'package:core_platform/core_platform.dart';
import 'package:feature_tuner/src/tuner_controller.dart';
import 'package:flutter/material.dart';

/// The microphone, from the user's side: why the tuner wants it, how to recover a refusal, and,
/// once granted, the control that starts and stops listening (`docs/PLATFORM_AUDIO.md` §2).
class MicrophonePrompt extends StatelessWidget {
  /// Creates the prompt for [state], acting through [controller].
  const new({required this.state, required this.controller, super.key});

  /// What the tuner knows and what the user asked of it.
  final TunerState state;

  /// Where the user's choices go.
  final TunerController controller;

  @override
  Widget build(BuildContext context) {
    if (state.permission == MicrophonePermissionStatus.granted) {
      final listening = state.listenWanted;
      return Semantics(
        toggled: listening,
        child: FilledButton.tonal(
          key: const Key('tuner.listenToggle'),
          onPressed: listening ? controller.stopListening : controller.listen,
          child: Text(listening ? 'Stop listening' : 'Start listening'),
        ),
      );
    }
    final (message, action, onPressed) = switch (state.permission) {
      MicrophonePermissionStatus.permanentlyDenied => (
        'The microphone is turned off for Diapason. Turn it on in Settings to tune.',
        'Open Settings',
        controller.openSettings,
      ),
      MicrophonePermissionStatus.denied => (
        'Diapason needs the microphone to hear your instrument.',
        'Try again',
        controller.listen,
      ),
      MicrophonePermissionStatus.notDetermined || MicrophonePermissionStatus.granted || null => (
        'Diapason listens to your instrument to show its pitch. '
            'Audio stays on your device: nothing is recorded or sent.',
        'Start listening',
        controller.listen,
      ),
    };
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Text(
          message,
          key: const Key('tuner.micMessage'),
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.bodyLarge,
        ),
        const SizedBox(height: 12),
        FilledButton(key: const Key('tuner.micAction'), onPressed: onPressed, child: Text(action)),
      ],
    );
  }
}
