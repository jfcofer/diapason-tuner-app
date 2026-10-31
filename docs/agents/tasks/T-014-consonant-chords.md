---
id: T-014
title: Two strings ringing at once, consonant intervals included, hold the last confident estimate
status: done
milestone: M2
owner: claude
created: 2026-10-24
---

## Goal

`PRODUCT_SPEC.md` §2 promises: "Two strings ringing at once: hold the last confident estimate
rather than alternating." `T-003c`'s octave guard (`adr/0029`) keeps chords from showing a third
note only for the families it gates: A2 over a bass E or a low B, and E4 over a low E, detuned
±30 ¢. It does not hold a consonant interval: a fourth or a fifth is periodic at the notes'
common period, so the tuner reads that period, clearly, as one note that is not sounding. This
pre-dates `T-003c`; `T-003c`'s review found it, and periodicity cannot fix it.

## Context

Read first:
- `docs/PRODUCT_SPEC.md` §2 (the edge cases with defined behaviour);
- `docs/AUDIO_ENGINE.md` §1 (the chord paragraph) and §3 (the octave guard, its rules after a
  note);
- `docs/adr/0029` (Consequences: the known gap).

Measured in `T-003c`'s review, from home Mid, the first note ringing loud under the second, at
48 and 44.1 kHz:
- A2 + D3 reads D1 (36.7 Hz), D3 + G3 reads G1 (49 Hz), and B3 + E4 reads E2 (82.4 Hz), each at
  clarity 1.0: the mixture *is* periodic there, so no NSDF test tells it from one note;
- G2 over a low B and C3 over a bass E read the old note's third sub-harmonic, and A2 + 60 to
  + 100 ¢ over a bass E reads 38–39 Hz, at clarity up to 0.97;
- E3 ringing loud over a low E: High shows the louder E3, octave known, before the chain moves.

Found in `T-020` (`adr/0031`): a high note over a second sound at a third of it (a ukulele's G4
over its body at 134.8 Hz and a sympathetic C4) reads that common period in Mid. `T-020` stopped
the verifier from moving the chain there. From silence, with the tone loud (6 dB down, or 11 dB
under a G4 that is a bare sine), High itself reads G4/3 and the chain still moves down.
`tests/chain.rs`'s `over_a_third` builds that signal.

The likely tool is time, not a single frame: detecting the onset of the plucked string (an energy
or spectral-flux rise), so the estimate of the string just plucked is held and a reading that
appears without an onset is not taken for a new note; or holding the last confident estimate
while the mixture is not clearly one note. It interacts with `T-003f`'s smoothing and lock: it
needs a decision on which of the two owns "hold", and an ADR.

## Acceptance criteria

- [x] `just verify` passes, and all six CI jobs are green (CI: on the PR).
- [x] A design note or ADR states how the tuner knows which string was just plucked, and what it
  shows while two strings ring.
- [x] *(with deviation: three ceilings, accepted by the owner 2026-10-31)* **The three consonant cases show no third note**, known or ambiguous, at both rates, with the
  first note ringing loud (τ 1.5 and 3 s) and 6 dB under: A2 → D3, D3 → G3, B3 → E4, sine,
  sawtooth and weak fundamental. `T-003c`'s judging rule (`tests/chain.rs`, `Judge`).
- [x] *(with deviation, as above)* The other loud chords above show no third note either: G2 over a low B, C3 over a bass E, A2
  + 60 to + 100 ¢ over a bass E, and E3 ringing over a low E shows no wrong octave.
- [x] **Measured and reported, not gated** (owner, 2026-10-30): fifths (D3 → A3, G3 → D4, the
  violin family's double stops), major thirds (G3 → B3, C4 → E4), two strings struck together from
  silence, and the gated chords detuned ±7, ±22 and ±45 ¢ (held out, to check the rule's margins).
  A row the rule cannot hold without regressing a gate gets a follow-up task with its numbers.
- [x] None of `T-003c`'s gates regress (leap sweep, downward changes, detuned gate, octave above a
  ringing string, latency from silence), and latency stays within §1's fail lines.

## Out of scope

- Polyphonic tuning (showing both notes): a product decision of its own.
- Recorded fixtures (`T-003e`), engine, FFI and UI (M3).

## Implementation notes

`adr/0033`. **What tells a chord from one string is the spectrum, not an onset.** A common period
has no fundamental of its own. The chain suspects a reading when the note locked before, still
sounding, is 3/2, 3 or 4 times it. It tests the reading's own peak with `pitch::partial_near`
(Hann DFT, ±150 ¢ grid) in the shortest window tail that resolves it. A failing suspect is
withheld, and from then on, until another note locks, every frame shown must repeat at 2τ. The
needle holds the old note's last value, flagged ambiguous (`Frame::held`), once the suspect has
failed for longer than its fundamental takes to resolve. Each step was measured, and the ones
dropped are in the ADR's rejected options and the journal.

**Deviations** (owner to accept or reject):
- Criteria 3 and 4 hold except for three residuals, each 1–2 frames, gated at their measured
  counts (`Gate::ChordCeiling`) so they cannot grow. **Accepted by the owner on 2026-10-31.**
  - D3 → G3, two sines, D3 6 dB under: one raw High frame, 21 ms after the onset;
  - A2 → D3, two sines, A2 6 dB under: the needle 60 ¢ behind the converging new note, 1–2 frames,
    at 47.5 ms;
  - C3 → bass E, τ 1.5 s, 44.1 kHz: one raw frame 51 ¢ sharp of the bass E, at 1.39 s.
  Tried for the first: `AFTER_NOTE_CLARITY` 0.88 fixed one rate and broke a recording.
- One `T-003c` figure moved, inside its fail line: E4 → low E, 12 dB under, at 44.1 kHz, needle
  58.7 → 61.3 ms, total 112.1 → 114.6 ms (target 110, fails at 150). `AUDIO_ENGINE.md` §1 updated.
- The plan's "octave below revokes known" step was not needed: E3 over a low E already showed no
  wrong frame, and it is now gated (`Gate::NoWrongFrame`).
- Measured rows that still fail go to `T-021`: fifths up (536–547 frames) and two strings struck
  together from silence (126–547 frames). Held-out detunings ±7, ±22 and ±45 ¢, fourths and
  fifths down, and major thirds up show none, except E4 → B3 at 6 dB under (3–4 frames).

## Verification performed

- **Baseline, before any change:** every gated chord showed a third note (113–573 frames a case;
  sine A2 → D3 289 known).
- **After:** every gated row passes at both rates. All 20 `recorded.rs` rows are identical to
  `main`. Every other figure `tests/chain.rs` prints was diffed line by line against `main`:
  - unchanged except T-003c's recorded-only detuned chords (third notes to 0, from up to 96);
  - T-020's G4 over G4/3 + 54 ¢ with C4 (348 → 432 known frames);
  - the A2 → bass E chord's needle showing the held A2 (allowed by its gate);
  - the one latency figure above.
- Clippy (pedantic) clean; `no_alloc.rs` gains a chord stream, asserted to hold frames.
- **Review** (`reviewer`): the latch survived another note locking, so a re-plucked old note was
  held at once. Fixed (`release_chord`) and gated: `the_hold_of_two_strings_lets_go` (chord,
  new note alone, old note alone, silence, old note: nothing held, no wrong needle after).
- `chain/chord` 105 µs per 10 ms block, against `chain/analysis` 80.4 µs (baseline 77.7): busy
  host, reported, not gated.
