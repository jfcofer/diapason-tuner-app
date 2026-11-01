---
id: T-024
title: Record the owner's four decisions, and reorder the work around a real note on a phone
status: done
milestone: M2
owner: claude
created: 2026-11-01
---

## Goal

After 283 commits the app could not show a note: nothing outside its own tests and benches calls
`dsp::chain`. The queue (`T-022`, `T-003d`, `T-023`, `T-003g`) would have kept it that way. The owner
reviewed an audit and decided four things on 2026-11-01. Write them down, so any agent can carry on
from the repo alone.

## Context

The decisions: a real note on the Redmi comes next; the strobe follows Peterson's asymmetric sweep;
the tuner listens after one consent; and 1.0 is Android only. Read `docs/adr/0035`, `docs/adr/0036`,
and `T-033` for the strobe brief.

## Acceptance criteria

- [x] `just docs-check` passes. No code changed; the six CI jobs are the PR's own gate to merge.
- [x] `adr/0035` and `adr/0036` are written and indexed.
- [x] `AGENTS.md` §1, `ROADMAP.md`, `PRODUCT_SPEC.md`, `TESTING.md` §7, `PLATFORM_AUDIO.md` and
      `CI_RELEASE.md` agree with them. L11 is `open`, owed by `T-031`.
- [x] Every next step has a task file with acceptance criteria: `T-025`–`T-033`, `T-003d`, `T-003g`.
- [x] `T-023` and `T-003g` are blocked on `T-026`; `T-017` on a decision to do iOS.
- [x] `STATE.md` names the new order.
- [x] The owner's reference photos stay out of the public repo (`.git/info/exclude`, local).

## Out of scope

Any code. `T-022`'s and `T-025`'s fixes, which have their own PRs.

## Implementation notes

**Deviation from the approved plan: `T-002c` is `abandoned`, not `blocked`.** `docs-check` does not let a
parent be done with an open slice, so a blocked `T-002c` would have kept `T-002` open for ever. That was
the very flaw `adr/0035` fixes. Its file records how a later iOS effort restarts. `T-017` has no parent,
and stays blocked as planned.

**Correction to the approved plan: `ssh-add` is not needed.** The plan inferred that `session-start`'s
failed fetch came from an empty ssh-agent. Plain and batch-mode fetches then both succeeded with the agent
still empty, so that inference was wrong and the cause is unknown. `T-022` says so.

**`T-003d` was split,** as the skeleton needs only 12-TET: temperaments became `T-003h`, a slice row.

The photos are Peterson's marketing images and the repo is public, so `T-033` carries what is taken from
them as text. `.git/info/exclude` is local: another clone has no such folder to exclude.

## Verification performed

- 2026-11-01: `just docs-check` and `tools/pr-check.sh main` on this branch. `git status` shows no photos.
- Docs only: no code, tests or generated files changed.
