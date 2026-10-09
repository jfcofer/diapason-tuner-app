---
id: T-006
title: Close M0 and gate task hygiene mechanically
status: done
milestone: M0
owner: claude
created: 2026-10-09
---

## Goal

Close M0 honestly against `docs/ROADMAP.md`, delete the bootstrap guide that only made sense
before it, and turn two conventions that are currently trusted into checks: task front matter and
criteria that a `done` task leaves unticked, and the per-dependency justification `adr/0009`
requires. The memory system is only as good as the facts in it; a convention nobody checks rots.

## Context

Read first: `docs/ROADMAP.md` (M0 exit), `docs/agents/tasks/README.md` (statuses), `T-001`
(criteria closed as unverifiable), `tools/docs-check.sh` (style to match), `adr/0009`.

## Acceptance criteria

- [x] M0 is marked closed in `ROADMAP.md`, with the one unverifiable exit criterion named and why
- [x] `docs/BOOTSTRAP.md` is deleted and nothing links to it
- [x] `docs-check` fails when a task's `id:` does not match its file name
- [x] `docs-check` fails on a `status:` outside `todo|in-progress|blocked|done|abandoned`
- [x] `docs-check` fails when a `done` task has an unticked criterion not marked "closed"
- [x] `docs-check` fails when `STATE.md` names a `done` or `abandoned` task as active
- [x] Each of the four checks above was seen failing on a deliberately broken file, then restored
- [x] A pull request template asks for the task, the gate, dependency justifications, ADRs and
      whether the RT path is touched
- [x] `just verify` green

## Out of scope

Any engine work (`T-002a`). CI workflow changes (`T-004`). Validating ADR bodies beyond what
`docs-check` already does.

## Implementation notes

- The criteria check judges a whole criterion, including its indented continuation lines, because
  the "closed" note usually lands on a wrapped line (`T-001` does this).
- **Its first run caught `T-001a`:** marked `done` with all seven boxes unticked, although its
  verification table records every outcome. Reconciled from that table. Measured results are
  ticked. Device run and iOS archive are marked closed, because `T-001` settled them. Spike cleanup
  is closed as no longer inspectable. No outcome was rewritten.
- The active task is the first `docs/agents/tasks/T-*.md` path in `STATE.md`, the same rule
  `tools/session-end.sh` uses, so the two cannot disagree.

## Verification performed

- Four throwaway files exercised the rules: an id mismatch, `status: finished`, a done task with
  one open and one closed wrapped criterion, and `STATE.md` pointing at `T-001`. Each rule failed
  with its own message, the closed criterion was accepted, and `docs-check` exited 1. After
  restoring, it exited 0 with the tree clean.
- `shellcheck tools/docs-check.sh` clean. `just verify` exited 0 on 2026-10-09, with this branch
  at `9d897fd`.
