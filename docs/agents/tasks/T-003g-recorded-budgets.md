---
id: T-003g
title: Budgets for recorded bass, electric guitar, ukulele and violin
status: blocked
milestone: M3
owner: unassigned
created: 2026-11-01
---

**Blocked on phone recordings of these instruments, and on `T-035`** (re-scoped 2026-11-02). `T-026` shows a guitar through a
phone failing in ways no library recording did, with its first two partials balanced differently (up to 17 dB on
G3). A budget set from libraries alone would promise nothing, so these budgets need phone takes as well.

## Goal

`AUDIO_ENGINE.md` §1 budgets recorded accuracy only for acoustic guitar. Others run under ceilings in
`recorded.rs`, every median within 2.6 ¢ of the consensus. Set their budgets by ADR, from measurements.

## Context

Read: `docs/adr/0030`, `T-003e`, `rust/crates/dsp/tests/recorded.rs`. Moved out of `T-003`'s M2 slices on
2026-11-01 (`T-024`): it is not an M2 exit criterion.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] An ADR sets a budget per instrument, each checked in `recorded.rs` as a gate.

## Out of scope

New detection logic.

## Implementation notes

## Verification performed
