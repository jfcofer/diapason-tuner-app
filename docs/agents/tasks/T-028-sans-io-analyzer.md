---
id: T-028
title: Analyse the ring's blocks, sans-IO, and publish a reading
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

Turn `T-027`'s blocks into pitch readings with no thread of its own, so the whole path is testable
offline. `engine` gains an `Analyzer` whose `pump()` drains the ring, as `session`'s `Supervisor` is a
sans-IO state machine with a driver around it.

## Context

Read: `docs/AUDIO_ENGINE.md` §2–§3, `docs/adr/0022`, `docs/adr/0028`, `docs/adr/0032`,
`rust/crates/dsp/src/chain.rs` (`Analysis`, `Frame`), `rust/crates/engine/clippy.toml`.

- **The thread cannot live in `engine`:** its `clippy.toml` bans `thread::sleep` and `Instant::now`
  crate-wide, suppressions are rejected, and `chain.rs` and `adr/0032` assumed otherwise. Correct them.
- `Analysis` is built off the RT path from the first block's rate. A gap in `frame` pushes zeros as
  `GateState::Closed`: it keeps `Frame.end` on the stream clock and resets the guard, since `Analysis`
  has no `reset`.
- **`unpitched`** returns to the callback by a `triple_buffer<bool>`: level-based, latest-wins.
- **Readings** reach `Engine` by a second triple buffer. `EngineSnapshot` keeps its single writer.
- **The reading carries:** `end`, rate, raw and display Hz, clarity, octave, held, gate, and two running
  phase integrals, `cycles += f · Δframes / rate`, one over raw and one over display, per analysed
  frame. Integrating in Dart from 60 Hz snapshots would discard most of the 400 frames/s (`T-032`).
- **`engine` gains `adr/0021`'s panic denials**, as `dsp` and `session` have.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] Offline: a sine through callback, ring and `pump()` reads within ±0.5 ¢ at both rates, and a
      recording from `fixtures/` matches what `tests/recorded.rs` gets from `Analysis` directly.
- [ ] Noise above the gate raises it through the returned `unpitched` flag.
- [ ] An overflow keeps `Frame.end` on the stream clock and does not show a wrong note.
- [ ] Sampled at 60 Hz, a reading shows no flicker that the 400 Hz frames did not have.
- [ ] Phase integrals advance by `f · t` within a stated tolerance on a steady tone.
- [ ] `adr/0037` covers the analyzer and who owns what.

## Out of scope

The thread and cadence (`T-029`). The FFI (`T-030`). The note the reading is nearest: Dart's (`T-031`).

## Implementation notes

## Verification performed
