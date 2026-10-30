# 0031 — A verifier moves the chain down to a third or a quarter only of a pitch the range cannot see

**Status:** Accepted · 2026-10-30 (`T-020`, made its own task by the owner on 2026-10-30). Revised
before merge, on `T-020`'s branch, after its review (k = 2 exempted); the decision date stands.

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

- **When the current pitch is 3 or 4 times a verifier's, the verifier moves the chain down only if
  the current range cannot see that pitch** (`RangeSelector::observe_verifier`): High below
  106.9 Hz (107.04 at 44.1 kHz), Mid below 50.2 Hz (50.3). `Mpm` takes the first NSDF peak within
  0.9 of the highest. A range that sees a period and still read one 3 or 4 times shorter therefore
  found the shorter one about as periodic. For one string that takes nearly all its energy in every
  third or fourth partial; two sounds whose common period it is do it easily.
- **Twice stays a hidden fundamental, seen or not.** A string whose odd partials are weak is about
  as periodic at half its period (4.7 % of the energy in odd partials: 0.906 at τ/2), so High reads
  2f₀ though it sees f₀, and only the verifier moves the chain to f₀.
- A low note the current range sees and reads is reached as before, by its own reading, through
  `RangeSelector::observe`.
- **This supersedes in part `adr/0029`**: its verifier bullet now carries this condition. The rest
  of 0029 stands.

## Consequences

- **Measured:** `ukulele_g4` now has no gross frame (it had 20) and `recorded.rs` gates that. Its
  median moved from −2.583 to −2.577 ¢. Ten of its frames are now `ambiguous octave`, still showing
  G with the right cents: at 1.250 s High's pick flips to G4/2 and the guard's τ/2 bridge shows the
  held G4 (`adr/0029` (a)). Every other recorded row is unchanged.
- **Synthetic** (`tests/chain.rs`): a G4 with a strong or gentler third partial, over a tone at G4/3
  (in tune, +30 or +54 ¢, 11 dB down), with or without a C4 20 dB down, at both rates. Before the
  fix, 20 of 24 cases showed a known wrong note (up to 577 frames). After it, none do, the chain
  never leaves High, and G4 is known from the first frame.
- **Unchanged:** every figure `tests/chain.rs` reports from before (diffed line by line), so
  `AUDIO_ENGINE.md` §1 and `adr/0029` stand.
- **Known gap, `T-014`:** a second tone at a third of a note is a chord. Loud enough (6 dB down, or
  11 dB under a G4 that is a bare sine), High itself reads the common period, and the chain moves
  down through `observe`. As `adr/0029` says of consonant chords, no periodicity test separates
  that from one note.
- **Found, not new:** the weak-odd string above shows 2f₀ as known for its first 16–17 judged
  frames (to 60–64 ms) before Mid moves the chain: High tests the octave below 2f₀ itself and
  finds no note there. Recorded by `a_note_with_weak_odd_partials_reaches_its_octave_from_high`.

## Options rejected

- **Visibility for every multiple, k = 2 included** (this ADR's first draft): the weak-odd string
  at 107–150 Hz (guitar A2 to D3) then showed 2f₀ as known for the whole note, 521–593 frames,
  where it had been right after 64 ms. The review found it.
- **Evidence in the current range's NSDF at the verifier's period** (`Mpm::peak_near`): an
  in-tune tone at G4/3 makes the mixture exactly periodic there, as periodic as a true f₀, so it
  cannot tell the two apart.
- **A guard rule after the switch** (a pick whose third multiple is the locked note): the common
  period repeats at 2τ, so `REPEATS` cannot reject it, and once the chain has switched the evidence
  (High seeing the period and reading G4) is gone.
- **Smoothing** (`T-003f`): it would hide 20 frames, not stop a range switch that should not happen.
