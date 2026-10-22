import 'package:feature_tuner/src/tuner_controller.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Tells [TunerController] whether the tuner is on screen: its route is the current one and its
/// tickers run. A route pushed over the tuner, or an offstage shell branch, hides it.
///
/// It uses only the framework's own signals, so the feature does not depend on the router.
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
    // Both lookups subscribe this state to changes, so this runs again whenever either changes.
    final visible =
        (ModalRoute.isCurrentOf(context) ?? true) && TickerMode.valuesOf(context).enabled;
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
