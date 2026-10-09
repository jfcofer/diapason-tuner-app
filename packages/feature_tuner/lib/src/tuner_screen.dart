import 'package:feature_tuner/src/engine_status_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The tuner screen.
///
/// In `T-001` this exists to prove one thing end to end: a value computed in `rust/crates/dsp`,
/// carried up through `engine` and `ffi`, across the flutter_rust_bridge boundary, into a widget.
/// Everything visual about it is temporary.
class TunerScreen extends ConsumerWidget {
  /// Creates the tuner screen.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final status = ref.watch(engineStatusProvider);
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Diapason')),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
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
              const SizedBox(height: 16),
              Text(
                status.running ? 'stream running' : 'no stream yet (T-002)',
                style: textTheme.bodySmall,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
