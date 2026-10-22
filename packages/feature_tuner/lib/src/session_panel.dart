import 'dart:math' as math;

import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:feature_tuner/src/microphone_prompt.dart';
import 'package:feature_tuner/src/tuner_controller.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The microphone flow and the live session: development scaffolding until the M3 tuner UI.
///
/// It rebuilds on every snapshot, about 30 times a second, because it prints the input level as
/// text. The real level meter will be a painter fed from the stream (`docs/ARCHITECTURE.md` §5).
class SessionPanel extends ConsumerWidget {
  /// Creates the panel.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tuner = ref.watch(tunerControllerProvider);
    final snapshot = ref.watch(sessionSnapshotProvider).value;
    final controller = ref.read(tunerControllerProvider.notifier);
    final diagnostics = ref.watch(showEngineDiagnosticsProvider);

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        MicrophonePrompt(state: tuner, controller: controller),
        if (snapshot != null) ...[
          const SizedBox(height: 24),
          _Readout(snapshot: snapshot, diagnostics: diagnostics),
        ],
        if (snapshot?.inputFault != null &&
            tuner.permission == MicrophonePermissionStatus.granted) ...[
          const SizedBox(height: 12),
          FilledButton(
            key: const Key('tuner.retryMic'),
            onPressed: controller.retryMicrophone,
            child: const Text('Retry microphone'),
          ),
        ],
        if (diagnostics) ...[
          const SizedBox(height: 24),
          OutlinedButton(
            key: const Key('tuner.testTone'),
            onPressed: controller.toggleTestTone,
            child: Text(tuner.toneWanted ? 'Stop test tone' : 'Play A4 test tone'),
          ),
        ],
      ],
    );
  }
}

class _Readout extends StatelessWidget {
  const new({required this.snapshot, required this.diagnostics});

  final SessionSnapshot snapshot;
  final bool diagnostics;

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final rms = snapshot.inputRms;
    final level = rms > 0
        ? '${(20 * math.log(rms) / math.ln10).toStringAsFixed(1)} dBFS'
        : 'silence';
    final lines = <String>[
      'Stream: ${snapshot.state.name} at ${snapshot.sampleRate} Hz',
      if (snapshot.inputActive) 'Microphone: $level' else 'Microphone: off',
      if (snapshot.diagnostics.requestedInputPreset case final requested? when diagnostics)
        'Preset: ${requested.name} requested, ${_presetName(snapshot.diagnostics)} obtained',
      if (snapshot.callbackLoad case final load? when diagnostics)
        _callbackLine(snapshot.diagnostics.worstCallback, load),
      if (snapshot.diagnostics.bufferFrames case final buffer? when diagnostics)
        _bufferLine(buffer, snapshot.diagnostics),
      if (snapshot.inputFault case final fault?) 'Microphone unavailable: ${fault.name}',
      if (snapshot.fault case final fault?) 'Audio stopped: ${fault.name}',
      if (snapshot.rebuilds > 0) 'Audio route changed ${snapshot.rebuilds}×',
    ];
    return Column(
      key: const Key('tuner.readout'),
      mainAxisSize: MainAxisSize.min,
      children: [for (final line in lines) Text(line, style: textTheme.bodyMedium)],
    );
  }
}

/// The preset obtained, by name, or the platform's own code when it is one this app never asks for.
String _presetName(StreamDiagnostics diagnostics) => switch (diagnostics.inputPreset) {
  null => 'unknown',
  InputPreset.other => 'code ${diagnostics.inputPresetCode}',
  final preset => preset.name,
};

/// The slowest callback, and its share of the time a burst allows (`docs/AUDIO_ENGINE.md` §1).
String _callbackLine(Duration worst, double load) =>
    'Callback: worst ${worst.inMicroseconds} µs, ${(load * 100).toStringAsFixed(1)} % of a burst';

/// The output buffer as grown so far (`docs/adr/0025`), and the microphone frames drained at start.
String _bufferLine(int buffer, StreamDiagnostics diagnostics) =>
    'Buffer: $buffer of ${diagnostics.bufferCapacityFrames ?? '?'} frames, '
    '${diagnostics.inputFramesShed} input frames shed';
