# 2026-11-01 — t-022-never-pushed

**Agent:** Claude Code (Opus 5.5)  **Task:** T-022  **Milestone:** M2

## Done
- `session-start` and `session-end` name a branch that was never pushed, and `session-end` also says when
  the branch is main or already merged. After a failed fetch, `session-start`'s verdicts say they may be
  stale. Both read one rule, in `tools/branch-state.sh`.

## Tried and abandoned
- **"No upstream" as the rule,** which is what the task asked for. It misses every branch started with
  `git checkout -b x origin/main`, as that sets `origin/main` as the upstream. Found by the demo.

## Surprises
- A demo script was denied as one large command, as it deleted a directory, turned off hooks and pushed to a
  ref named `main`, all in a scratch repo. Rewritten without those, as a script with an explicit local remote.
- `safe.bareRepository=explicit` on this host refuses `git -C <bare>`: use `--git-dir`.

## Left for next session
- `T-025`.
