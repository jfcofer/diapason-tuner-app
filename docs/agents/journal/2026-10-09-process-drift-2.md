# 2026-10-09 — process-drift-2

**Agent:** Claude Code (Opus 5.5)  **Task:** T-010, plans T-011 and T-002b part 2b
**Milestone:** M1

## Done
- Reconciled STATE.md with `session-start`. PR #13 had merged part 2a, but STATE still said "not
  pushed". Dependabot #2 and #10 are green and wait on the owner.
- Planned what follows with the owner. The order is T-010, then T-011 (ktfmt), then 2b-i
  (capabilities, preset, permission), 2b-ii (stream tuning), and part 3. The owner chose
  **Pigeon** for platform channels, and **ktfmt as its own task** before any real Kotlin lands.
- **T-010:** fixed two checks that had been passing while checking nothing, added a parent/slice
  rule to `docs-check`, and corrected T-002, T-002b and `CLAUDE.md`. Each fix was proven on a
  planted failure.

## Tried and abandoned
- **Catching T-002's drift in `session-start` by matching PR branches.** No branch carries
  `/T-002-`, only `/T-002b-`, so matching branch names could not have caught it. Comparing a
  parent with its slices, offline in `docs-check`, can.
- **Adding an `ask` rule for `build.gradle.kts` to `.claude/settings.json`.** The permission
  classifier refused the agent's edit. That is the right boundary, so the rule is left to the
  owner. Until it exists, `CLAUDE.md` says to ask before touching the signing block.

## Surprises
- **The TODO check in `docs-check` had never worked.** A space after `\K` meant it matched nothing,
  so `TODO(T-999)` passed. The pattern now proves itself on a known line before it scans the tree.
- **`session-end` passed this session before any session file was touched.** It compared
  against `HEAD~1`, which was the previous session's STATE commit. It now compares against the
  mark `session-start` now leaves in the git dir, and it sees root files and quoted paths.
- **The reviewer caught half of what was left.** The first fix still let a `git grep` without PCRE
  pass silently, and still missed root files. Its findings are in T-010's notes.
- **Undoing a probe with `git checkout <file>` reverted that file's uncommitted edits too.**
  Re-applied. Probes now run in a throwaway local clone.
- **`git fetch` failed in `session-start`** (`Permission denied (publickey)`): the SSH agent had
  no key loaded. It fell back as designed, and `gh` still saw the PRs.

## Left for next session
- The owner: push, open the PR, merge it. Then add the `settings.json` `ask` rule.
- `T-011` ktfmt. Maven Central, checked 2026-10-09: 0.64 (2026-06-24) is the latest release. The
  jar is `ktfmt-0.64-with-dependencies.jar` (71 MB), with a published `.sha256`.
