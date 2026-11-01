---
id: T-023
title: Fifths and unequal strums from silence show no third note
status: todo
milestone: M2
owner: unassigned
created: 2026-10-31
---

## Goal

`T-021` (`adr/0034`) withholds fourths, thirds and two sines struck together from silence. Three
held-out families still show a third note. They are what a player does most: strumming open
strings, and a fiddler's double stop started from silence.
- **Fifths struck together** (D3 + A3, G3 + D4): their common period sounds 2f and 3f, as a
  string's attack with a weak fundamental does. 522–545 frames.
- **Unequal strums with a weak fundamental**, the lower string 6 dB under (A2 + D3, B3 + E4,
  D3 + G3): 534–575 frames.
- **Two sines a fourth apart, staggered 20 ms** (B3 + E4): 25–26 frames.

## Context

Read first: `docs/adr/0034` (its rejected options, which are this task's traps), `docs/adr/0033`,
and `docs/AUDIO_ENGINE.md` §3 (two strings ringing).

What a fix must not repeat:
- Telling a fifth from a string by 5f, which lies under the decimator's edge from 150 Hz up
  (−9 dB at 980 Hz). It blanked the gold nylon D3 and steel G3 during their attacks.
- Any test that rejects while undecided, or under a range fixed by hand.

Leads, not decisions:
- Time: a string's weak fundamental appears within about 100 ms, and a strum's common period
  never sounds one. But showing nothing during that wait costs single notes.
- A fifth from silence whose upper string locks first becomes `0033`'s case (ratio 3).

Measure on the gold recordings frame by frame, not only inside their judged window.

## Acceptance criteria

- [ ] `just verify` passes, and all six CI jobs are green.
- [ ] The three families show no third note, or a measured ceiling the owner accepts. They are
  moved from measured to gated (`tests/chain.rs`).
- [ ] Every `T-021` figure is unchanged, or the change is recorded. That covers the recordings
  frame by frame (nylon D3, steel G3, pizzicato G3 with High fixed), and the gates for a note
  after a strum, High fixed, and no fundamental.

## Out of scope

- Polyphonic tuning (showing both notes). The engine, the FFI and the UI.

## Implementation notes

## Verification performed
