---
id: T-014
title: Two strings ringing at once, consonant intervals included, hold the last confident estimate
status: todo
milestone: M2
owner: unassigned
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

The likely tool is time, not a single frame: detecting the onset of the plucked string (an energy
or spectral-flux rise), so the estimate of the string just plucked is held and a reading that
appears without an onset is not taken for a new note; or holding the last confident estimate
while the mixture is not clearly one note. It interacts with `T-003f`'s smoothing and lock: it
needs a decision on which of the two owns "hold", and an ADR.

## Acceptance criteria

- [ ] `just verify` passes, and all six CI jobs are green.
- [ ] A design note or ADR states how the tuner knows which string was just plucked, and what it
  shows while two strings ring.
- [ ] **The three consonant cases show no third note**, known or ambiguous, at both rates, with the
  first note ringing loud (τ 1.5 and 3 s) and 6 dB under: A2 → D3, D3 → G3, B3 → E4, sine,
  sawtooth and weak fundamental. `T-003c`'s judging rule (`tests/chain.rs`, `Judge`).
- [ ] The other loud chords above show no third note either: G2 over a low B, C3 over a bass E, A2
  + 60 to + 100 ¢ over a bass E, and E3 ringing over a low E shows no wrong octave.
- [ ] None of `T-003c`'s gates regress (leap sweep, downward changes, detuned gate, octave above a
  ringing string, latency from silence), and latency stays within §1's fail lines.

## Out of scope

- Polyphonic tuning (showing both notes): a product decision of its own.
- Recorded fixtures (`T-003e`), engine, FFI and UI (M3).

## Implementation notes

## Verification performed
