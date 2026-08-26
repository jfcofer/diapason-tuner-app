import 'package:flutter/material.dart';

/// Diapason's Material theme.
///
/// Deliberately thin in `T-001`: the scaffold task explicitly ships no design tokens, so this is
/// stock Material 3 with a seed colour and nothing else. Replacing it with the real token set is
/// milestone M3's job, and doing it early would mean rebuilding it around whatever the engine turns
/// out to be able to provide (`docs/BOOTSTRAP.md`).
abstract final class DiapasonTheme {
  /// Seed colour for both brightness variants until real tokens exist.
  static const Color _seed = Color(0xFF2E7D6F);

  /// The light theme.
  static ThemeData light() => _build(Brightness.light);

  /// The dark theme.
  static ThemeData dark() => _build(Brightness.dark);

  static ThemeData _build(Brightness brightness) => ThemeData(
    colorScheme: ColorScheme.fromSeed(seedColor: _seed, brightness: brightness),
    useMaterial3: true,
  );
}
