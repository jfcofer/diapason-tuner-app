---
id: T-019
title: Check the session protocol on every PR's diff, so it binds every agent
status: todo
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

The session protocol (`AGENTS.md` §2) is enforced only by `just session-end`, which an agent must
remember to run, and only Claude has a command that does. Bookkeeping drifted after a merge three
times: `T-008` (fixed by `T-009` with a reminder), then `T-003c`, whose task stayed `in-progress`
and whose STATE said "PR not yet opened" after #34 merged. `docs-check` passed on that tree. Move the
checks from the session, which only some agents run, to the PR diff, which every agent and human
produces, and run them in CI.

## Context

Read: `AGENTS.md` §2, `docs/agents/CONTEXT_SYSTEM.md`, `tools/session-end.sh`, `tools/docs-check.sh`
(its self-test pattern), `.github/workflows/ci.yml` (`checks` job), `lefthook.yml`,
`.github/pull_request_template.md`, `docs/CI_RELEASE.md` §1. The ruleset requires six named jobs:
add steps, never a job.

## Acceptance criteria

- [ ] `tools/pr-check.sh BASE` and `just pr-check BASE` check a task branch (`*/T-###*-slug`)
  against `BASE`:
  - code changed ⇒ the branch's task file changed;
  - exactly one `docs/agents/journal/*.md` added, and no existing entry modified;
  - `docs/agents/STATE.md` changed;
  - the branch's task is `done`, or `in-progress` with a `**Next part:**` line.
  Dependabot and branches with no task ID pass with a note.
- [ ] `tools/session-end.sh` calls `pr-check.sh` for the checks they share: one home for each rule.
- [ ] `pr-check.sh --self-test` builds throwaway branches in a temporary clone and fails each rule
  once: code without a task edit, an edited journal entry, no STATE edit, an `in-progress` task
  with no `**Next part:**`. `just verify` runs the self-test.
- [ ] CI's `checks` job runs `just pr-check origin/<base>` on `pull_request` events only.
- [ ] `docs-check` fails when STATE or a task cites a `T-###x` ID that has neither a file nor a row
  in its parent's slice table.
- [ ] `lefthook` pre-commit rejects a staged archive (`*.zip`, `*.7z`, `*.tar*`), and any file over
  1 MiB outside `fixtures/audio/`. `.gitignore` lists `sound-fixtures/`.
- [ ] Claude adapter: a `SessionStart` hook in `.claude/settings.json` runs `just session-start`.
- [ ] `CONTEXT_SYSTEM.md`, `AGENTS.md` §2 and the PR template say the gate is CI.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

Checking prose for truth (STATE's PR claims stay reconciled by `session-start`). New CI jobs.
Hooks for other agents' tools: their adapters point at `AGENTS.md`, and CI is the enforcement.

## Implementation notes

## Verification performed
