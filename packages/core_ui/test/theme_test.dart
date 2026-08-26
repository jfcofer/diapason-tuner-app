import 'package:core_ui/core_ui.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('both themes are Material 3 and carry the matching brightness', () {
    expect(DiapasonTheme.light().useMaterial3, isTrue);
    expect(DiapasonTheme.light().colorScheme.brightness, Brightness.light);
    expect(DiapasonTheme.dark().colorScheme.brightness, Brightness.dark);
  });
}
