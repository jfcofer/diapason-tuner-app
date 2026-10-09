# 2026-10-09 — session-start-review

**Agent:** Claude Code (Opus 5.5)  **Task:** T-007  **Milestone:** M1

## Done
- Fixed what the adversarial review of T-007 found in `tools/session-start.sh`:
  - merged branches are now detected by patch, not ancestry;
  - a missing `origin/main` is named;
  - network calls are bounded;
  - check verdicts are complete.
- The owner approved `session-start`'s two read-only network calls; `CLAUDE.md` § Permissions
  records it.
- STATE.md's ADR table was replaced by a pointer to `docs/adr/README.md`, the one home for that
  index.

## Tried and abandoned
- **Building T-008's release APK in a separate git worktree.** The worktree has no shared build
  cache, so cargokit recompiled the Rust stack with fat LTO for every ABI, alongside a subagent.
  The owner killed it for RAM. T-008's edits were saved as a patch, and the worktree was removed.
  Its build happens in the main checkout, one ABI at a time.
- **Re-enabling the three rules very_good_analysis 11 dropped.** All three are deprecated in Dart
  3.13, so re-enabling them only postpones the removal.

## Surprises
- **Rebase-merge plus `delete_branch_on_merge`** means a merged local branch is never an ancestor
  of `origin/main`. The first version of the script would have reproduced the stale-STATE incident
  it was written to prevent. The reviewer caught it before it shipped.

## Left for next session
- Push T-007 and open its PR (the owner approves).
- Then finish T-008 on `build/T-008-min-sdk-28`: its pin, docs and ADR are committed there. What
  remains is the one-ABI release build and `check-android-release`.
