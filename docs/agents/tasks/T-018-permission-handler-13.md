---
id: T-018
title: Move permission_handler to 13, which needs compileSdk 37
status: todo
milestone: M3
owner: unassigned
created: 2026-10-30
---

## Goal

`permission_handler` is held at 12, and Dependabot with it, because 13 needs compileSdk 37 and
this repo pins compileSdk 36 (`tools/versions.env`). Raising a pinned SDK level is its own task
and ADR (`AGENTS.md` §8), so the hold has an owner and an exit instead of waiting forever.

## Context

Read first: `tools/versions.env`; `docs/adr/0011` (pins) and `0019` (minSdk 28); `docs/adr/0024`
(the permission flow through Pigeon); `permission_handler` 13's changelog. Trap: `targetSdk` and
`minSdk` do not move as a side effect.

## Acceptance criteria

- [ ] An ADR records compileSdk 37: why, and what it does not change (targetSdk, minSdk)
- [ ] `tools/versions.env` and the Gradle files agree; `just doctor` enforces the new pin
- [ ] `permission_handler` is 13 and the microphone flow passes on the Redmi (`T-002d` rows)
- [ ] Dependabot's ignore rule for it is removed
- [ ] `just verify` passes, and all six CI jobs are green

## Out of scope

Raising targetSdk or minSdk.

## Implementation notes

## Verification performed
