---
id: T-019
title: Check the session protocol on every PR's diff, so it binds every agent
status: done
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

- [x] `tools/pr-check.sh BASE` and `just pr-check BASE` check a task branch (`*/T-###*-slug`)
  against `BASE`:
  - code changed ⇒ the branch's task file changed;
  - at least one `docs/agents/journal/*.md` added (one per session), and no existing entry edited;
  - `docs/agents/STATE.md` changed;
  - the branch's task is `done`, or `in-progress` with a `**Next part:**` line.
  Dependabot passes with a note; any other branch must name its task (`AGENTS.md` §7).
- [x] `tools/session-end.sh` calls `pr-check.sh` for the checks they share: one home for each rule.
- [x] `pr-check.sh --self-test` builds throwaway branches in a temporary clone and fails each rule
  once: code without a task edit, an edited journal entry, no STATE edit, an `in-progress` task
  with no `**Next part:**`. `just verify` runs the self-test.
- [x] CI's `checks` job runs `just pr-check origin/<base>` on `pull_request` events only.
- [x] `docs-check` fails when STATE or a task cites a `T-###x` ID that has neither a file nor a row
  in its parent's slice table.
- [x] `lefthook` pre-commit rejects a staged archive (`*.zip`, `*.7z`, `*.tar*`), and any file over
  1 MiB outside `fixtures/audio/`. `.gitignore` lists `sound-fixtures/`.
- [x] `CONTEXT_SYSTEM.md`, `AGENTS.md` §2 and the PR template say the gate is CI.
- [x] `just verify` passes, and all six CI jobs are green (the ruleset merges nothing otherwise).

## Out of scope

Checking prose for truth (STATE's PR claims stay reconciled by `session-start`). New CI jobs.
Hooks for other agents' tools: their adapters point at `AGENTS.md`, and CI is the enforcement.

## Implementation notes

- **Rules:** `tools/pr-check.sh`. In PR mode it diffs `BASE...HEAD`, committed only. In
  `--session` mode, `session-end` runs it on the session's diff plus uncommitted work, for the rules
  they share: STATE updated, code recorded in a task file, the journal append-only. `session-end`
  keeps only what is per-session (STATE dated today, today's journal entry, an active task file).
  It dropped its STATE line-budget check, which `docs-check` already runs.
- **Naming is enforced:** in PR mode, a branch that names no task fails, except `dependabot/*`.
  `AGENTS.md` §7 already required the name; nothing checked it.
- **CI:** the `checks` job checks out with `fetch-depth: 0` and runs `pr-check` on pull requests.
  It passes the branch through `env`, not inline `${{ }}`, because a branch name is
  attacker-controlled text.
- **Deviation, journal:** "exactly one entry" became "at least one": a multi-session PR adds one
  per session (#36 adds two).
- **Deviation, dropped:** a `SessionStart` hook in `.claude/settings.json`. Auto mode refuses an
  agent editing its own settings, which is right. The owner can add it (`"SessionStart":
  [{"matcher": "startup", "hooks": [{"type": "command", "command": "just session-start"}]}]`). It
  is a convenience; CI is the enforcement.
- **The lefthook guard is POSIX sh**, since lefthook may run it under `dash`.
- Dry run: `#36`'s branch passes. `T-003c`'s branch would have failed rule 4: `in-progress`, with
  no next part, on its own merged PR. That is the drift this task exists for.

## Verification performed

2026-10-30, host. `pr-check --self-test`: 13 cases, 5 passing branches and 8 failing ones, one per
rule. `pr-check` passes on #36's branch and on this one. `session-end` passes via `--session`.
`docs-check` fails on a planted `T-777` in STATE. The lefthook guard refuses a staged 2 MB file and
a `.zip`.
