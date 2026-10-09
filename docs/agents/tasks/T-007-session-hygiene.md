---
id: T-007
title: Reconcile out-of-band repo state at every session start
status: in-progress
milestone: M1
owner: claude
created: 2026-10-09
---

## Goal

Stop `STATE.md` from silently going stale when something changes *between* sessions. On
2026-10-09 it said two branches were "awaiting push and PR" hours after the owner had merged both
as PRs #6 and #7. Meanwhile three Dependabot PRs sat open for five days with one failing.
Merges, Dependabot and CI all happen outside a session, so no snapshot can be trusted on them. The
session start has to reconcile them mechanically, for every agent, not just Claude.

## Context

Read: `AGENTS.md` §2, `docs/agents/CONTEXT_SYSTEM.md`, `justfile` (`session-start`),
`.claude/commands/session-start.md`.

- Dependabot PRs: #2 (thiserror) and #3 (dart-minor) are green.
- #4 (very_good_analysis 11) fails `dart` and `checks` on formatting drift only: its formatter
  settings rewrite generated FRB output and two `core_domain`/`audio_engine` files.

## Acceptance criteria

- [x] `just session-start` prints open PRs with their check status, and says so when `gh` is
      unavailable instead of failing
- [x] `just session-start` prints branches, local and remote, not merged into `main`
- [x] `AGENTS.md` §2 starts with "run `just session-start`", within the 200-line budget
- [x] `.claude/commands/session-start.md` defers to the recipe rather than duplicating it
- [x] `STATE.md` states the rule: PRs by number, merge state is reconciled at session start
- [x] `STATE.md` matches the live repo (merged PRs, the Dependabot gap answered)
- [x] Dependabot #4's drift fixed by formatting, with no lint weakened and no `// ignore`
- [ ] `just verify` green; six CI jobs green on the PR (local gate green; PR not yet pushed)

## Out of scope

Merging the Dependabot PRs (the owner merges). `minSdk` (`T-008`). Any audio work (`T-002b`).

## Implementation notes

- **`tools/session-start.sh`** replaces the inline recipe, matching `session-end.sh`.
  - It runs `git fetch --prune` first, so the remote branches it reports are current.
  - Each PR's checks collapse into one verdict in gh's own `--jq`: any failure is red; green needs
    every check to be success, skipped or neutral; anything else is pending. A first attempt
    parsed the conclusions in bash and called green PRs pending.
  - No `gh`, or no network: it prints a notice and carries on. It is never a hard failure.
  - Shellcheck-clean without suppressions.
- **very_good_analysis 11:**
  - `trailing_commas` goes from `preserve` to `automate`, plus Dart 3.13 rules
    (`unnecessary_type_name_in_constructor` → `new(...)` constructors, `use_declaring_parameters`,
    and others).
  - It also drops `one_member_abstracts`, `unnecessary_await_in_return` and
    `avoid_private_typedef_functions`. Its README lists all three as deprecated in Dart 3.13, so
    this follows the SDK and weakens nothing. Re-enabling deprecated rules would only defer their
    removal.
  - Done on this branch rather than on Dependabot's, which is five days behind `main`.
  - Landing the version on `main` closes #4.
- **Adversarial review** (the `reviewer` subagent) found, and this branch fixes:
  - **Rebase-merge rewrites SHAs**, so `--no-merged` would call every merged branch unmerged
    forever, which is the very failure this task exists for. Branches are now compared by patch
    with `git cherry`, a deleted upstream is shown, and recently merged PRs are listed.
  - **No `origin/main`** used to print "none". It now says the comparison is impossible.
  - **Verdicts:** legacy `state` statuses, `ERROR`, `STARTUP_FAILURE` and a null rollup were
    misread.
  - **Hangs:** network calls ran with no prompt or timeout guard. They are now bounded
    (`BatchMode`, `GIT_TERMINAL_PROMPT=0`, `timeout 20` where available), and gh's first error line
    is shown.
  - **Permissions:** the network reads ran unprompted through the `just *` allowance. The owner
    approved this explicitly, and `CLAUDE.md` § Permissions records it.
  - **STATE** asserted "awaits push", the same kind of claim T-007 exists to stop. It now lists
    branches in flight and defers their state to `session-start`. The ADR table duplicated
    `docs/adr/README.md` and became a pointer, which leaves 8 lines of headroom.
- **Moved out of STATE.md:** the `T-002b` plan now lives in that task's Implementation notes
  (one fact, one home), which keeps STATE within its 120 lines.

## Verification performed

- `tools/session-start.sh` on 2026-10-09: it listed the three Dependabot branches, with #4 red and
  #2/#3 green. Those verdicts match `gh pr view` on each.
- `shellcheck tools/session-start.sh`: clean.
- `just verify`: green. `check-drift` was also run after staging, to prove `gen` is idempotent
  under the new formatter. CI's `check-drift` on a fresh clone is the stronger proof, and it runs on
  the PR.
- Scratch repos: a rebase-merged branch (a cherry-picked patch with a new SHA) reports "merged
  into main" while an open one reports "1 commit(s) not in main". With `origin` removed, both
  failures are named.
- Verdict jq against synthetic data: null rollup → pending, legacy `ERROR` → failing,
  `STARTUP_FAILURE` → failing, in progress → pending, mixed check/status success → green.
