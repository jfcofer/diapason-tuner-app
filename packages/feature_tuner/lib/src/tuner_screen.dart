import 'package:audio_engine/audio_engine.dart';
import 'package:feature_tuner/src/session_panel.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The tuner screen.
///
/// Until the M3 tuner UI, this is development scaffolding for the audio spine: which Rust builds
/// are running, the microphone permission flow, and the live session. Everything visual about it
/// is temporary.
class TunerScreen extends ConsumerWidget {
  /// Creates the tuner screen.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(engineStatusProvider);
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Diapason')),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text('Engine reports', style: textTheme.labelLarge),
                const SizedBox(height: 12),
                Text(
                  status.dspBuild,
                  key: const Key('tuner.dspBuild'),
                  style: textTheme.headlineSmall,
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 4),
                Text(status.engineBuild, style: textTheme.bodyMedium, textAlign: TextAlign.center),
                const SizedBox(height: 32),
                const SessionPanel(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
