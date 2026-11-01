import 'dart:math' as math;

import 'package:meta/meta.dart';

/// The note number of A4, on the MIDI scale.
const int a4Note = 69;

/// How far below a tie, in semitones, a frequency still goes to the upper note.
///
/// Exactly between two semitones, the logarithms of Dart and Rust can differ in their last bit and
/// round apart. This is about a ten-millionth of a cent from the tie, far below any reading.
const double _tieSemitones = 1e-9;

const List<String> _englishNames = <String>[
  'C',
  'C#',
  'D',
  'D#',
  'E',
  'F',
  'F#',
  'G',
  'G#',
  'A',
  'A#',
  'B',
];

/// A frequency's nearest note (`docs/AUDIO_ENGINE.md` §4).
///
/// `rust/crates/dsp` implements the same maths, and both assert every row of
/// `fixtures/note_table.json` (`docs/adr/0007`).
@immutable
final class NearestNote {
  /// A note [number] and how many [cents] the frequency is from it.
  const new({required this.number, required this.cents});

  /// The note shown, on the MIDI scale: the sounding note plus the transposition.
  final int number;

  /// How far the frequency is from the sounding note, from -50 up to, but not including, +50.
  ///
  /// Sharp is positive. A transposition does not change it.
  final double cents;

  /// The note's scientific pitch octave: middle C is in octave 4.
  int get octave => (number ~/ 12) - 1;

  /// The note's English letter name and accidental, without its octave.
  ///
  /// Sharps only. Other name systems are the display's concern.
  String get englishName => _englishNames[number % 12];

  @override
  bool operator ==(Object other) =>
      other is NearestNote && other.number == number && other.cents == cents;

  @override
  int get hashCode => Object.hash(number, cents);

  @override
  String toString() => 'NearestNote($englishName$octave, $cents cents)';
}

/// The note [frequencyHz] is nearest under a reference [a4Hz], shown with [transposition] semitones
/// added.
///
/// A tie goes to the upper note. Null unless both frequencies are positive and finite, as there is
/// no note to name.
NearestNote? nearestNote(double frequencyHz, {double a4Hz = 440, int transposition = 0}) {
  if (!frequencyHz.isFinite || !a4Hz.isFinite || frequencyHz <= 0 || a4Hz <= 0) {
    return null;
  }
  final semitones = a4Note + 12 * (math.log(frequencyHz / a4Hz) / math.ln2);
  final sounding = (semitones + 0.5 + _tieSemitones).floor();
  return NearestNote(number: sounding + transposition, cents: 100 * (semitones - sounding));
}
