# 2026-10-30 — process-drift-5

**Agent:** Claude Code (Opus 5.5)  **Task:** T-019  **Milestone:** M2

## Done
- `tools/pr-check.sh`: the session protocol checked on a branch's diff, which CI's `checks` job runs
  on every pull request. A PR must touch its task file if code changed, update STATE, add a journal
  entry without editing an old one, and leave its task `done` or naming its next part.
- `session-end` delegates the shared rules to it, so each rule has one home.
- `docs-check` fails on a cited task ID with no file and no slice row.
- A lefthook guard refuses archives, and files over 1 MiB outside `fixtures/audio/`.
- Planned `T-003e` (task file, licence audit, traps) and a `T-003g` slice row for non-guitar
  budgets.

## Tried and abandoned
- "Exactly one journal entry per PR": a multi-session PR legitimately adds several.
- A Claude `SessionStart` hook: auto mode refuses an agent editing its own settings. Left to the
  owner, with the snippet in the task notes.

## Surprises
- `AGENTS.md` §7's branch naming was never checked. It now is, because pr-check needs the task ID.

## Left for next session
- After #36 merges: rebase this branch onto `main`, push it, open its PR. Then `T-003e`.
