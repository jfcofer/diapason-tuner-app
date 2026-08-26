import 'package:feature_metronome/feature_metronome.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('MetronomeScreen renders its placeholder', (tester) async {
    await tester.pumpWidget(const MaterialApp(home: MetronomeScreen()));

    expect(find.text('Metronome'), findsOneWidget);
    expect(find.text('Not implemented yet'), findsOneWidget);
  });
}
