import 'package:feature_settings/feature_settings.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('SettingsScreen renders its placeholder', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: SettingsScreen()));

    expect(find.text('Settings'), findsOneWidget);
    expect(find.text('Not implemented yet'), findsOneWidget);
  });
}
