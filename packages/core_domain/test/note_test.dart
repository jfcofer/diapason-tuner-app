import 'dart:convert';
import 'dart:io';

import 'package:core_domain/core_domain.dart';
import 'package:test/test.dart';

/// The table holds six decimal places, so half of the sixth, and a little for the arithmetic.
const double _centsTolerance = 1e-6;

void main() {
  test('every row of the table shared with dsp holds', () {
    final table = jsonDecode(
      File('../../fixtures/note_table.json').readAsStringSync(),
    ) as Map<String, Object?>;
    final rows = (table['rows']! as List<Object?>).cast<Map<String, Object?>>();
    expect(rows.length, greaterThan(100), reason: 'the table lost its rows');

    for (final row in rows) {
      final got = nearestNote(
        (row['frequency_hz']! as num).toDouble(),
        a4Hz: (row['a4_hz']! as num).toDouble(),
        transposition: row['transposition']! as int,
      );
      expect(got, isNotNull, reason: '$row');
      expect(got!.number, row['midi'], reason: 'the note, at $row');
      expect(
        got.cents,
        closeTo((row['cents']! as num).toDouble(), _centsTolerance),
        reason: 'the cents, at $row',
      );
      expect('${got.englishName}${got.octave}', row['name'], reason: '$row');
    }
  });

  test('what is not a frequency names no note', () {
    expect(nearestNote(0), isNull);
    expect(nearestNote(-1), isNull);
    expect(nearestNote(double.nan), isNull);
    expect(nearestNote(double.infinity), isNull);
    expect(nearestNote(440, a4Hz: 0), isNull);
    expect(nearestNote(440, a4Hz: double.nan), isNull);
  });

  test('transposition moves the note and not the cents', () {
    final concert = nearestNote(445)!;
    final shown = nearestNote(445, transposition: -12)!;
    expect(shown.number, concert.number - 12);
    expect(shown.cents, concert.cents);
  });
}
