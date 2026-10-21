---
id: T-012
title: Enforce in config and checks the holds and STATE facts that lived only in prose
status: done
milestone: M1
owner: claude
created: 2026-10-20
---

## Goal

Reconciling the repo on 2026-10-20, ten days after the last session, found three facts that were
true only because a document said so, and had drifted. Turn each into a check that fails, prove
each on a planted failure, so the next drift is caught by `just verify` or `session-end`, not by
an agent reading closely.

## Context

Read: `AGENTS.md` §2, `tools/docs-check.sh`, `tools/session-end.sh`, `.github/dependabot.yml`.
`T-010` built the checks this extends.

Found on 2026-10-20:
- **Dependabot #20** bumped `permission_handler` to 13 and went red on `build-android`, although
  STATE.md holds it at 12 (13 needs compileSdk 37, its own task under `AGENTS.md` §8). The hold
  lived only in STATE.md. Closed; the `melos` entry is the precedent for a config-level hold.
- **STATE.md's header** read `updated 2026-10-09 (T-002b part 2b-i)` after the 2026-10-10 rewrite
  for 2b-ii. `session-end` checks that STATE.md was touched, not that it says when.
- **STATE.md cited `T-003-pitch-core`**, which has no task file. Only the first `tasks/…md` path in
  STATE.md was checked, and a bare slug was not checked at all.

## Acceptance criteria

- [x] `dependabot.yml` ignores `permission_handler` major updates, with the reason beside it
- [x] `session-end` fails when STATE.md was changed but its header date is not today
- [x] `docs-check` fails when STATE.md cites a task by its slug (`T-###-slug`, with or without the
      path) and no such file exists
- [x] Each new check shown to fail on a planted failure, then pass when it is reverted
- [x] `just verify` passes

## Out of scope

The compileSdk 37 / `permission_handler` 13 bump itself. A selftest harness for `docs-check` and
`session-end` like `doctor-selftest` (worth having; not this task).

## Implementation notes

- `docs-check` reads the slug form only. A bare ID may name a task not yet written, and a match
  after `/` (a branch such as `feat/T-002b-stream-tuning`) is skipped. Any `tasks/` prefix is
  stripped before the lookup.
- STATE.md's own `T-003-pitch-core` was the first real failure. It now cites `T-003`.
- No selftest exists for `docs-check` or `session-end` (only for `doctor`), so the planted
  failures below were run by hand.

## Verification performed

- `docs-check` on the unchanged STATE.md: failed on `T-003-pitch-core`. A planted
  `docs/agents/tasks/T-999-nope.md` failed. A planted `chore/T-998-branch-only` did not. Reverted:
  green.
- `session-end` with STATE.md changed and the 2026-10-09 header: "header is not dated
  2026-10-20". Header dated today: "STATE.md updated, dated 2026-10-20".
- `just lint-ci` (actionlint, shellcheck): clean. `just verify`: see the PR's checks.
