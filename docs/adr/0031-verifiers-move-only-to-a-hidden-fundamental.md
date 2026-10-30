# 0031 — A verifier moves the chain down only to a fundamental the current range cannot see

**Status:** Accepted · 2026-10-30 (`T-020`, made its own task by the owner on 2026-10-30)

## Context

`adr/0029` analyses the ranges below the selected one as verifiers. A verifier's pitch moves the
chain down when the current range's pitch is 2, 3 or 4 times it (`range::MAX_HARMONIC`), with fresh
frames that agree. The rule assumes the verifier has read a fundamental the current range cannot
see, such as a low E with a weak fundamental, which High hears as its second partial.

`T-003e`'s recordings broke that assumption. On `ukulele_g4` (392 Hz, home High), from 1.1 s the
spectrum holds two more sounds under G4's first partial: a tone at 134.8 Hz, 11 dB down (the body,
54 ¢ above G4/3), and the C4 string ringing in sympathy at 261.5 Hz, 21 dB down (G4's second partial
is C4's third). Mid, the verifier, reads their near-common period at 130.7 Hz, clarity 0.85–0.90.
G4 is 3 × 130.7 Hz, so at 1.200 s two fresh agreeing Mid frames moved the chain to Mid. Mid can test
the octave of 130.7 Hz, so 20 frames (1.2025–1.250 s) showed a C3 with its octave **Known**, until
the watcher brought the chain back. `octave::OctaveGuard::must_repeat` tests only the locked note's
octaves, and a common period repeats anyway, so nothing downstream could catch it.

High sees down to 106.9 Hz (`RangeTable::sees`) and read G4 throughout.

## Decision

- **A verifier moves the chain down only to a pitch the current range cannot see**
  (`RangeSelector::observe_verifier`): High below 106.9 Hz (107.0 at 44.1 kHz), Mid below 50.2 Hz
  (50.3). `Mpm` takes the first NSDF peak within 0.9 of the highest. So a range that sees a period
  and still read one 2 to 4 times shorter found the shorter one about as periodic, and a hidden
  fundamental is not: a weak fundamental's second partial reads only 0.55–0.65 of the note
  (`octave::COMPARABLE`), and a range that sees such a note reads the note itself. The longer period
  belongs to a second sound.
- A low note the current range sees is reached by the range's own reading, as any note is: below
  its band, through `RangeSelector::observe`, once it has held for a window.
- **This supersedes in part `adr/0029`**: its verifier bullet ("a verifier ... reads it in a window
  started after the note") now has this condition added. The rest of 0029 stands.

## Consequences

- **Measured:** `ukulele_g4` now has no gross frame (it had 20) and `recorded.rs` gates that. Its
  median moved from −2.583 to −2.577 ¢. Ten of its frames are now `ambiguous octave`, still showing
  G with the right cents: at 1.250 s High's pick flips to G4/2 and the guard's τ/2 bridge shows the
  held G4 (`adr/0029` (a)). Every other recorded row is unchanged.
- **Synthetic** (`tests/chain.rs`, `a_high_note_over_a_tone_at_a_third_of_it_never_hands_down_to_that_tone`):
  a G4 with a strong or a gentler third partial, over a tone at G4/3 (in tune, +30 ¢ or +54 ¢,
  11 dB down), with or without a C4 20 dB down, at both rates. Before the fix, 20 of the 24 cases
  showed a known wrong note (up to 577 frames). After it, none do, and the chain never leaves High.
- **Unchanged:** every gated figure in `tests/chain.rs`, and so in `AUDIO_ENGINE.md` §1 and
  `adr/0029` (diffed line by line). Weak-fundamental and pinch notes at 51.9–73.4 Hz from Mid and at
  108–146.8 Hz from High, from silence and after a note two octaves up, arrive with identical
  latencies. Only the ungated `T-014` chords (A2 ringing loud over a bass E) moved, by one to eight
  frames either way.
- **Known gap, `T-014`:** a second tone at a third of a note is a chord. Loud enough (6 dB down, or
  11 dB under a G4 that is a bare sine), High itself reads the common period, and the chain moves
  down through `observe`. As `adr/0029` says of consonant chords, no periodicity test separates
  that from one note.

## Options rejected

- **Applying the condition only to k = 3 and 4:** a second sound an octave below a note the range
  sees gives the same false evidence at k = 2. Every gated test passed with the general rule, so the
  narrower one bought nothing.
- **A guard rule after the switch** (a pick whose third multiple is the locked note): the common
  period repeats at 2τ, so it would have needed a new test unique to this case, and the evidence
  (High seeing the period and reading G4) is gone once the chain has switched.
- **Smoothing** (`T-003f`): it would hide 20 frames, not stop a range switch that should not happen.
