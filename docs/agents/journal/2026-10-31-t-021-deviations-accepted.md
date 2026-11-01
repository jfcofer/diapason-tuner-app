# 2026-10-31 — t-021-deviations-accepted

**Agent:** Claude Code (Opus 5.5)  **Task:** T-021  **Milestone:** M2

## Done
- The owner accepted all of `T-021`'s deviations after #43 merged:
  - the fifths up keep their ceilings with the old note 6 dB under;
  - fifths and unequal strums struck together from silence wait for `T-023`;
  - seven detuned E4 → low E rows have a few more frames on the ringing E.

  Recorded in the task file and STATE.
- Deleted the superseded branch `feat/T-021-fifths-and-strums`, at the owner's request. It never
  had a PR.

## Tried and abandoned
- Recording the acceptance on #43's branch: #43 had already merged. The commit landed on local
  `main`, whose push the ruleset rejected. It moved to its own branch, and local `main` went
  back to `origin/main`.

## Surprises
- Twice this session the owner merged a PR, and once switched the working copy to `main`, while
  an agent was working. **Check `git branch --show-current` and the PR's state before every
  commit and push.** A rule like that belongs in `T-022`'s `session-end` notice.

## Left for next session
- `T-022`, then `T-003d`, `T-023`.
