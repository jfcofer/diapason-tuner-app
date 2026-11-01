---
id: T-022
title: Say when a finished branch was never pushed, so STATE's "merged" cannot go unopened
status: done
milestone: M2
owner: claude
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

Read: `tools/session-start.sh` (the "Branches" loop over `git for-each-ref`), `tools/session-end.sh`, `AGENTS.md` §2, and `docs/agents/CONTEXT_SYSTEM.md`.
Both scripts must stay bash 3.2-portable and never fatal on network failure (`T-007`). The
`pr-check` self-test (`just pr-check-selftest`) is the pattern for proving a rule fails.

**Traps, found 2026-11-01 before any work started (`T-024`):**
- `%(upstream:track)` is empty both for a branch in sync with its upstream and for one with no
  upstream, so it cannot tell them apart. Test `%(upstream)`.
- These verdicts rest on refs from `git fetch`, which can fail. On 2026-11-01 `session-start` printed
  `Permission denied (publickey)`, then the same fetch succeeded minutes later with no change on this
  side; the cause is unknown. When the fetch failed, say so beside any verdict that read those refs.
- A branch pushed without `-u` has no upstream while the remote has it. Compare with
  `refs/remotes/origin/<branch>` too, or it reads as never pushed.

## Acceptance criteria

- [x] `just verify` passes. CI's six jobs are this PR's own gate to merge.
- [x] `just session-start` prints `never pushed: open its PR` for a local branch with commits not
  in `origin/main` and no upstream. It prints nothing new for a branch whose upstream exists or
  that is merged. Shown on a scratch clone in the PR's verification.
- [x] `just session-end` prints the same as a notice, never a failure, when the current branch
  has commits not in `origin/main` and no upstream.
- [x] `just session-end` also notices, never fails, when the current branch is `main` or is
  already merged (`git cherry` empty against `origin/main`). The owner merging, or switching the
  working copy, mid-session put a commit on local `main` on 2026-10-31.
- [x] After a failed fetch, the verdicts above say they may be stale. Shown on the scratch clone.
- [x] A branch pushed without `-u` does not read as never pushed. Shown on the scratch clone.
- [x] `shellcheck` stays clean. No new network call: only refs `git fetch` already brought.

## Out of scope

- Pushing or opening PRs automatically: anything that writes to GitHub still asks (`CLAUDE.md`).
- Changing what STATE records on a PR (`AGENTS.md` §2 is right as written).

## Implementation notes

**The criterion's "no upstream" was too narrow, and the demo caught it.** `git checkout -b x origin/main`
sets `origin/main` as x's upstream without saying so, so the first version never flagged such a branch.
The rule is now: no upstream other than `origin/main`, no `refs/remotes/origin/<branch>`, and commits
not in `origin/main`.

Both scripts source `tools/branch-state.sh`, so the rule has one home. A new file was the alternative to
copying it into both. `grep -c`, not `grep -q`: under `pipefail`, an early exit could kill `git cherry`
with SIGPIPE and read as "no commits".

`session-end` makes no network call, so its notices say "as of the last fetch" rather than knowing
whether one failed. The cause of 2026-11-01's failed fetch stays unknown.

No self-test was added: these are notices, not gates, and the criteria ask for a scratch clone.

## Verification performed

2026-11-01, on a throwaway clone with a local bare remote, simulating a rebase-merge. The two lines each
for the real branches and `origin/landed` are left out, and long lines are cut:

```
=== session-start, fetch succeeds
Branches
  from-local-main                                  1 commit(s) not in main, never pushed: open its PR
  from-origin-main                                 1 commit(s) not in main, never pushed: open its PR
  in-sync                                          merged into main
  pushed-with-u                                    1 commit(s) not in main
  pushed-without-u                                 1 commit(s) not in main
  rebase-merged                                    merged into main
  origin/pushed-with-u                             1 commit(s) not in main
  origin/pushed-without-u                          1 commit(s) not in main

=== session-end on from-origin-main:   ! from-origin-main was never pushed: open its PR (as of the last fetch)
=== session-end on from-local-main:    ! from-local-main was never pushed: open its PR (as of the last fetch)
=== session-end on pushed-with-u:    (no notice)
=== session-end on pushed-without-u: (no notice)
=== session-end on rebase-merged:      ! rebase-merged is already merged: new work needs a new branch (as of t
=== session-end on in-sync:            ! in-sync has no commits of its own yet (as of the last fetch)
=== session-end on main:               ! on main: work lands through a PR, so move it to a branch (as of the l
=== session-start, fetch fails
Branches
  git fetch failed, so remote branches may be stale: fatal: '<scratch>/t022.j0c0zK/does-not-exist.git' does no
  from-local-main                                  1 commit(s) not in main, never pushed: open its PR (may be 
  from-origin-main                                 1 commit(s) not in main, never pushed: open its PR (may be 
  in-sync                                          merged into main (may be stale: the fetch failed)
  pushed-with-u                                    1 commit(s) not in main (may be stale: the fetch failed)
  pushed-without-u                                 1 commit(s) not in main (may be stale: the fetch failed)
  rebase-merged                                    merged into main (may be stale: the fetch failed)
  origin/pushed-with-u                             1 commit(s) not in main (may be stale: the fetch failed)
  origin/pushed-without-u                          1 commit(s) not in main (may be stale: the fetch failed)

```

`shellcheck tools/*.sh` is clean, and `just verify` passes. The notices do not touch `FAILURES`.
