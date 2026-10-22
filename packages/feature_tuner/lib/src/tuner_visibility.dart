import 'package:feature_tuner/src/tuner_controller.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Tells [TunerController] whether the tuner is on screen, which is whether its tickers run.
///
/// The navigator stops them under an opaque route once it covers the tuner, and an offstage shell
/// branch stops them too; a dialog, a menu or a bottom sheet over the tuner does not, so the tuner
/// keeps listening under its own controls (`docs/DESIGN_SYSTEM.md` puts them in a sheet). It uses
/// only the framework's own signal, so the feature does not depend on the router.
class TunerVisibility extends ConsumerStatefulWidget {
  /// Wraps [child], the tuner's content.
  const new({required this.child, super.key});

  /// The tuner's content.
  final Widget child;

  @override
  ConsumerState<TunerVisibility> createState() => _TunerVisibilityState();
}

class _TunerVisibilityState extends ConsumerState<TunerVisibility> {
  var _visible = true;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // The lookup subscribes this state to changes, so this runs again whenever it changes.
    final visible = TickerMode.valuesOf(context).enabled;
    if (visible == _visible) return;
    _visible = visible;
    // A provider must not change while widgets build, so report it once this frame is done.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) ref.read(tunerControllerProvider.notifier).setVisible(visible: visible);
    });
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
