---
id: T-034
title: Keep merged PRs out of STATE, and let a session see an attached device
status: done
milestone: M2
owner: claude
created: 2026-11-01
---

## Goal

A session on 2026-11-01 found `STATE.md` wrong in two ways within minutes of it merging: "Open: none"
with three Dependabot PRs open, and an active task (`T-027`) that its own "Next up" put second, behind
`T-026`. Nothing could have caught either. PR numbers belong to `session-start`, which reads them live,
and a snapshot that names the wrong active task misleads every later session. Fix both at the cause, and
clear the small drift found in the same reconciliation.

## Context

Read: `AGENTS.md` §2, `docs/agents/CONTEXT_SYSTEM.md`, `tools/docs-check.sh`, `tools/session-start.sh`.
Earlier passes of this kind: `T-009`, `T-010`, `T-012`, `T-015`, `T-019`, `T-022`.

## Acceptance criteria

- [x] `just verify` passes; all six CI jobs green.
- [x] `docs-check` fails when `STATE.md` has a `#NN` outside its `**In flight:**` line, and when the task
      under "Active task" is not the first under "Next up". Each rule proves itself on planted text on
      every run, as the code-name pattern already does.
- [x] `STATE.md` names `T-026` active, and no longer lists merged tasks or PRs.
- [x] `session-start` prints attached devices and never fails on them (no `adb`, no server, no device).
- [x] `just run` defaults to Android (`adr/0035`).
- [x] No live pointer to "`STATE.md`, Traps" remains. `adr/0024` keeps its own: it is Accepted.

## Out of scope

Anything in `T-026`. Moving `.claude/agents/` somewhere every tool reads: other agents already get the
standards those briefs cite, through `AGENTS.md` and `.agents/skills/review`.

## Implementation notes

A `#NN` rule was chosen over banning the words "merged" or "open": it is the number that goes stale, and
prose about a PR with no number cannot be reconciled against anything anyway.

An active section naming no task passes, and so does an empty "Next up": either can be true between
milestones, and a rule that forces an invented task would be worse than the drift.

## Verification performed

- 2026-11-01: `tools/docs-check.sh` on this branch, with both planted-text self-checks passing.
- `tools/session-start.sh` with the Redmi attached, and with `adb` taken off `PATH`.
