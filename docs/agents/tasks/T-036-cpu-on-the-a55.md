---
id: T-036
title: Bring the tuner's CPU on an A55 within budget, or change the budget by ADR
status: blocked
milestone: M2
owner: unassigned
created: 2026-11-02
---

**Blocked on `T-035`,** which changes the path that costs the most. `T-027` waits on this task's decision.

## Goal

`chain/analysis` costs 13.4 % of an A55 against a target of 8 %, and `chain/chord` 15.9 % against a fail line of
15 % (`T-026`). `AUDIO_ENGINE.md` §1 cannot stay as it stands while the code does this. Either the cost falls, or
an ADR says what the budget means and why.

The figures move with conditions: one cooler pass read 12.5 % and 14.3 %, so the chord sits about on the line.

## Context

Read: `T-026`, `docs/adr/0028`, `docs/AUDIO_ENGINE.md` §1, `rust/crates/dsp/benches/chain.rs`.

- **Where 1338 µs per 10 ms goes, held in High** (arithmetic from frame costs, not a profile): Low verifier
  2 × 235, High 4 × 106, Mid verifier 4 × 48, the filters 163. Held in Mid, the same sum is about 8.3 %.
- **Hops are not the cheap lever.** Doubling High's hop adds 7.5 ms to its ledger line (one hop, two of smoothing):
  87.7 ms against a target of 85. Mid would reach 116 against 110.
- **Options to weigh:** run the Low verifier on fewer frames, which `T-035` may need more of; cut the filters,
  which cost 4.5 × as much on an A55 as an A76, where the rest cost about 3 ×; or judge the budget differently.
- **The bench cannot settle it alone:** it omits `Front` and everything above `dsp`, which add load, and it
  presumably holds the core near its top frequency, which real load would not. `T-031` can measure the real thread.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] `just bench-android --save` after `T-035`, recorded here, with temperature and screen state controlled
      and stated, and enough repeats to show the spread at those conditions.
- [ ] Each option's saving is measured, not estimated, and what it costs in latency or accuracy is recorded.
- [ ] The owner's decision is an ADR, and `AUDIO_ENGINE.md` §1 agrees with it and with the code.

## Out of scope

The analysis thread itself (`T-029`). Pinning a thread to a core, unless an ADR chooses it.

## Implementation notes

## Verification performed
