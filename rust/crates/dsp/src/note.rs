//! The note a frequency is nearest, and how far from it (`docs/AUDIO_ENGINE.md` §4).
//!
//! Equal temperament, a reference A4 and a transposition. `core_domain` implements the same maths in
//! Dart, and both assert every row of `fixtures/note_table.json` (`docs/adr/0007`). No allocation:
//! the engine's analysis side calls it per frame.

use crate::convert::floor_to_i32;

/// The note number of A4, on the MIDI scale.
pub const A4_NOTE: i32 = 69;

/// How far below a tie, in semitones, a frequency still goes to the upper note. Exactly between two
/// semitones, the two languages' logarithms can differ in their last bit and round apart. This is
/// about a ten-millionth of a cent from the tie, far below any reading.
const TIE_SEMITONES: f64 = 1e-9;

/// A frequency's nearest note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestNote {
    /// The note shown, on the MIDI scale: the sounding note plus the transposition.
    pub note: i32,
    /// How far the frequency is from the sounding note, from −50 up to, but not including, +50.
    /// Sharp is positive. A transposition does not change it.
    pub cents: f64,
}

/// The note `frequency_hz` is nearest under a reference `a4_hz`, shown with `transposition`
/// semitones added. A tie goes to the upper note. `None` unless both frequencies are positive and
/// finite, as there is no note to name.
///
/// ```
/// use diapason_dsp::note::nearest;
///
/// let a4 = nearest(440.0, 440.0, 0).unwrap();
/// assert_eq!(a4.note, 69);
/// assert!(a4.cents.abs() < 1e-9);
///
/// // A concert B♭ on a B♭ clarinet shows as C.
/// assert_eq!(nearest(466.1638, 440.0, 2).unwrap().note, 72);
/// assert_eq!(nearest(0.0, 440.0, 0), None);
/// ```
#[must_use]
pub fn nearest(frequency_hz: f64, a4_hz: f64, transposition: i32) -> Option<NearestNote> {
    if !(frequency_hz.is_finite() && a4_hz.is_finite() && frequency_hz > 0.0 && a4_hz > 0.0) {
        return None;
    }
    let semitones = f64::from(A4_NOTE) + 12.0 * (frequency_hz / a4_hz).log2();
    let sounding = floor_to_i32(semitones + 0.5 + TIE_SEMITONES);
    Some(NearestNote {
        note: sounding.saturating_add(transposition),
        cents: 100.0 * (semitones - f64::from(sounding)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_not_a_frequency_names_no_note() {
        for (frequency, reference) in [
            (0.0, 440.0),
            (-1.0, 440.0),
            (f64::NAN, 440.0),
            (f64::INFINITY, 440.0),
            (440.0, 0.0),
            (440.0, f64::NAN),
        ] {
            assert_eq!(nearest(frequency, reference, 0), None);
        }
    }

    #[test]
    fn transposition_moves_the_note_and_not_the_cents() {
        let concert = nearest(445.0, 440.0, 0).unwrap();
        let shown = nearest(445.0, 440.0, -12).unwrap();
        assert_eq!(shown.note, concert.note - 12);
        assert_eq!(shown.cents.to_bits(), concert.cents.to_bits());
    }
}
