---
id: T-021
title: Fifths up and two strings struck together show no third note
status: in-progress
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

`T-014` (`adr/0033`) holds the fourths, the major thirds, and the fourths and fifths down. Two
families it measured still show a third note. **Fifths up** (D3 → A3, G3 → D4): violin, viola,
cello and mandolin players tune by double stops in fifths. **Two strings struck together from
silence**, as a strum across them. Both are common enough to fix before M3 puts a needle on
screen.

## Context

Read first: `docs/adr/0033` (the rule and its rejected options), `docs/AUDIO_ENGINE.md` §3 (two
strings ringing, the octave guard), `docs/adr/0029` (the octave guard, which owns k = 2).

Measured in `T-014` (`tests/chain.rs`, `two_strings_ringing_show_no_third_note` and
`two_strings_struck_together_from_silence_are_measured`, sawtooth, from home Mid):
- D3 → A3 and G3 → D4, ringing at τ 1.5 s: 524–547 third-note frames, known. Their common period
  is half the lower note (D2, G2), the octave of the locked note: `adr/0029`'s domain, outside
  `octave::COMMON_RATIOS`. At 6 dB under, D3 → A3 is 274–281 and G3 → D4 0–4.
- Struck together: 126–547 frames for every pair and timbre. No note is locked, so no reading is
  suspected. The same spectral test may apply to any reading below both strings, but it must not
  cost single notes (owner, 2026-10-25), and a phone microphone may remove low fundamentals.

Also open: while two strings ring loud, the new note waits until the old one decays (more than
1.5 s for sines and at τ 3 s). Whether the product wants that shorter is the owner's question.

## Acceptance criteria

- [ ] `just verify` passes, and all six CI jobs are green.
- [ ] D3 → A3 and G3 → D4 (sine, sawtooth, weak; τ 1.5 and 3 s, 6 dB under; both rates) show no
  third note, by `T-014`'s rule (`Traps::third`).
- [ ] The six strum pairs show no third note, or a measured ceiling the owner accepts.
- [ ] Every `T-003c`, `T-014` and `recorded.rs` figure is unchanged, or the change is recorded.

## Out of scope

- Showing both notes (polyphonic tuning). The engine, the FFI and the UI (M3).

## Implementation notes

**Plan** (owner, 2026-10-31: from silence, show nothing until one string reads as one; `T-014`'s
three ceilings accepted, so this task must not grow them). One test, "does one string explain
this reading?", from the partials at f, 2f, 3f and 5f (`pitch::partial_near`):
- a string sounds its fundamental; one whose fundamental a microphone removed still sounds 2f, 3f
  and 5f; one read an octave low sounds 2f, and neither 3f nor 5f;
- a fifth (2 : 3) sounds 2f and 3f, not 5f; fourths (3 : 4) and thirds (4 : 5) not 2f; two sines
  read at their mean or a quasi-period sound none of them.

It is asked of a reading half the locked note (fifths up; the octave guard keeps every reading one
string explains) and of every reading while no note has locked (strums from silence).

## Verification performed
