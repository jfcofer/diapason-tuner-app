import 'package:core_platform/core_platform.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('the fake reports each change at once and remembers the last', () {
    final lifecycle = FakeAppLifecycle();
    final seen = <AppLifecycleState>[];
    lifecycle.changes.listen(seen.add);

    expect(lifecycle.current, AppLifecycleState.resumed);
    lifecycle
      ..set(AppLifecycleState.inactive)
      ..set(AppLifecycleState.hidden);

    expect(seen, [AppLifecycleState.inactive, AppLifecycleState.hidden]);
    expect(lifecycle.current, AppLifecycleState.hidden);
  });

  testWidgets('the Flutter lifecycle follows the binding', (tester) async {
    final lifecycle = FlutterAppLifecycle();
    addTearDown(lifecycle.dispose);
    final seen = <AppLifecycleState>[];
    lifecycle.changes.listen(seen.add);

    tester.binding
      ..handleAppLifecycleStateChanged(AppLifecycleState.inactive)
      ..handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    await tester.pump();

    expect(seen, [AppLifecycleState.inactive, AppLifecycleState.hidden]);
    expect(lifecycle.current, AppLifecycleState.hidden);
  });
}
