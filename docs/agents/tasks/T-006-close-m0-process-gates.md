---
id: T-006
title: Close M0 and gate task hygiene mechanically
status: in-progress
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

- [ ] M0 is marked closed in `ROADMAP.md`, with the one unverifiable exit criterion named and why
- [ ] `docs/BOOTSTRAP.md` is deleted and nothing links to it
- [ ] `docs-check` fails when a task's `id:` does not match its file name
- [ ] `docs-check` fails on a `status:` outside `todo|in-progress|blocked|done|abandoned`
- [ ] `docs-check` fails when a `done` task has an unticked criterion not marked "closed"
- [ ] `docs-check` fails when `STATE.md` names a `done` or `abandoned` task as active
- [ ] Each of the four checks above was seen failing on a deliberately broken file, then restored
- [ ] A pull request template asks for the task, the gate, dependency justifications, ADRs and
      whether the RT path is touched
- [ ] `just verify` green

## Out of scope

Any engine work (`T-002a`). CI workflow changes (`T-004`). Validating ADR bodies beyond what
`docs-check` already does.

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._
