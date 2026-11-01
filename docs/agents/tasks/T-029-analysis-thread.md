---
id: T-029
title: Give the analyzer a thread, and rebuild it with every stream
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

Run `T-028`'s `Analyzer` on a real thread owned by `session`, rebuilt with each open under
`adr/0022`'s replay rule, and publish readings at the tuner's 60 Hz (`adr/0028`).

## Context

Read: `docs/adr/0022`, `rust/crates/session/src/{driver,supervisor}.rs`, `docs/ARCHITECTURE.md` §4.

- **`driver.rs`'s `PUBLISH_INTERVAL` couples the tick to publishing, at 33 ms.** Separate them, or the
  ledger's 16.7 ms snapshot line cannot hold.
- **One analysis thread for the session's life.** Each open builds a fresh ring and `Analyzer`, passed
  to it by `mpsc`; an abandoned consumer drops its analyzer. It polls with `recv_timeout` at about a
  quarter of a burst, because the callback may wake nothing (`AGENTS.md` §6).
- **An `epoch` rises with every open,** since `rebuilds` counts only disconnects. A consumer must not
  compare `end` or phase across epochs.
- **Later settings** go through a triple buffer: `Command`s reach only `Processor`.
- **`session` may depend only on `engine` and `audio_io`** (`tools/check-deps.sh`), so reading types
  are re-exported through `engine`.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] Virtual-time tests in `session`: a reopen at 44.1 kHz rebuilds the analyzer at that rate; an
      output-only stream has none; a stop then a start raises `epoch`; no reading follows a stop.
- [ ] The cold snapshot's 33 ms cadence is unchanged. Readings publish every 16.7 ms while the input
      is live, and skip when `end` has not moved.
- [ ] `adr/0022`'s replay test covers the analyzer's settings.
- [ ] `adr/0037` is complete. `ARCHITECTURE.md` §4 says who owns the analysis thread.

## Out of scope

The FFI and Dart (`T-030`). Low-power mode's 20 Hz (L9, M3).

## Implementation notes

## Verification performed
