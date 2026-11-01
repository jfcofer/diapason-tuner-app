---
id: T-003g
title: Budgets for recorded bass, electric guitar, ukulele and violin
status: blocked
milestone: M3
owner: unassigned
created: 2026-11-01
---

**Blocked on `T-026`** (since 2026-11-01): real microphone captures show whether budgets set from
library recordings mean anything on a phone. Re-scope or abandon it from that evidence.

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
