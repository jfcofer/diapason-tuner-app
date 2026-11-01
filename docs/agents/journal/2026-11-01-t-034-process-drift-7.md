# 2026-11-01 — t-034-process-drift-7

**Agent:** Claude Code (Opus 5.5)  **Task:** T-034  **Milestone:** M2

## Done
- `docs-check` now rejects a PR number outside STATE's **In flight** line, and an active task that is not
  first in "Next up". Both rules test themselves on planted text on every run.
- `STATE.md` names `T-026` active and no longer lists merged work.
- `session-start` prints attached devices. `just run` defaults to Android. The dead "`STATE.md`, Traps"
  pointers now name `DEVELOPMENT.md` §6.

## Tried and abandoned
- A docs-check rule for `FILE.md §N` references: a one-off script found none dangling over living docs,
  so it would guard nothing today.
- Moving `.claude/agents/` to a tool-neutral folder: they are thin and cite docs every agent can read.

## Surprises
- `main`'s ruleset is strict about up-to-date branches, so a queue of green Dependabot PRs still costs one
  CI run each, in series. Clear the queue before opening a PR, which cannot be force-pushed to catch up.

## Left for next session
- `T-026`: benches on the Redmi, then the owner's captures.
