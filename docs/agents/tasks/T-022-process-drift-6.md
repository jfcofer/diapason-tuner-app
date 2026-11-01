---
id: T-022
title: Say when a finished branch was never pushed, so STATE's "merged" cannot go unopened
status: todo
milestone: M2
owner: unassigned
created: 2026-10-31
---

## Goal

The protocol has an agent write STATE "as it will be after the merge" (`AGENTS.md` §2). That is
right for a PR, but nothing says when the PR was never opened. `T-014` closed on 2026-10-30 with
STATE reading "merged", while its seven commits sat on a local branch with no upstream. The next
session found out only by comparing `git ls-remote` with the PR list (2026-10-31, PR #42).
`session-start` already prints "N commit(s) not in main" for that branch, but not that the
branch was never pushed. Name that case where the two session recipes already look.

## Context

Read: `tools/session-start.sh` (the "Branches" loop over `git for-each-ref`, which already reads
`%(upstream:track)`), `tools/session-end.sh`, `AGENTS.md` §2, and `docs/agents/CONTEXT_SYSTEM.md`.
Both scripts must stay bash 3.2-portable and never fatal on network failure (`T-007`). The
`pr-check` self-test (`just pr-check-selftest`) is the pattern for proving a rule fails.

## Acceptance criteria

- [ ] `just verify` passes, and all six CI jobs are green.
- [ ] `just session-start` prints `never pushed: open its PR` for a local branch with commits not
  in `origin/main` and no upstream. It prints nothing new for a branch whose upstream exists or
  that is merged. Shown on a scratch clone in the PR's verification.
- [ ] `just session-end` prints the same as a notice, never a failure, when the current branch
  has commits not in `origin/main` and no upstream.
- [ ] `just session-end` also notices, never fails, when the current branch is `main` or is
  already merged (`git cherry` empty against `origin/main`). The owner merging, or switching the
  working copy, mid-session put a commit on local `main` on 2026-10-31.
- [ ] `shellcheck` stays clean. No new network call: only refs `git fetch` already brought.

## Out of scope

- Pushing or opening PRs automatically: anything that writes to GitHub still asks (`CLAUDE.md`).
- Changing what STATE records on a PR (`AGENTS.md` §2 is right as written).

## Implementation notes

## Verification performed
