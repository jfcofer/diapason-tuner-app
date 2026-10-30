---
id: T-020
title: No wrong note shown as certain when a high note's range hands down to Mid
status: todo
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

`T-003e`'s recordings found a wrong note the octave guard marks as known. On `ukulele_g4` (392 Hz,
from High), the selector moves to Mid at 1.20–1.25 s. Mid then reads 131 Hz, a third of G4 (a
twelfth below, a C3), at clarity 0.85–0.90, with the octave marked **Known**: 20 frames that would
put a wrong note on screen as certain. The guard (`adr/0029`) checks octaves (`f/2`, `2f`); a
factor of 3 passes it. The owner made this its own task (2026-10-30).

## Context

Read: `docs/adr/0029-octave-guard.md`, `docs/AUDIO_ENGINE.md` §2–§4, `rust/crates/dsp/src/octave.rs`,
`rust/crates/dsp/src/chain.rs` (range selection), `rust/crates/dsp/tests/recorded.rs` (the
`ukulele_g4` row and its ceiling), `fixtures/audio/README.md`. Dsp work goes to the `dsp-engineer`
agent. Reproduce first: `cargo nextest run -p diapason_dsp --test recorded --no-capture`.

## Acceptance criteria

- [ ] The cause is named, with the frames it happens in: why the selector leaves High while G4 is
  still sounding, and why Mid's reading passes the guard as `Known`.
- [ ] A synthetic test reproduces it before the fix (a high note decaying across the High/Mid
  boundary, strong third partial) and passes after.
- [ ] `ukulele_g4` shows zero gross frames, and `recorded.rs` gates that, as it gates `bass_b0`.
- [ ] No other recorded fixture gains a gross frame, and no gold median moves past ±2 ¢.
- [ ] Latency budgets in `AUDIO_ENGINE.md` §1 still hold (`tests/chain.rs`). The bench gate holds.
- [ ] Any change to the guard's rules supersedes the relevant part of `adr/0029`.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

Budgets for the ukulele (`T-003g`). Smoothing, which would hide the symptom (`T-003f`).

## Implementation notes

## Verification performed
