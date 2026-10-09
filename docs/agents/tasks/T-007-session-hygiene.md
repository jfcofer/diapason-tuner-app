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

- [ ] `just session-start` prints open PRs with their check status, and says so when `gh` is
      unavailable instead of failing
- [ ] `just session-start` prints branches, local and remote, not merged into `main`
- [ ] `AGENTS.md` §2 starts with "run `just session-start`", within the 200-line budget
- [ ] `.claude/commands/session-start.md` defers to the recipe rather than duplicating it
- [ ] `STATE.md` states the rule: PRs by number, merge state is reconciled at session start
- [ ] `STATE.md` matches the live repo (merged PRs, the Dependabot gap answered)
- [ ] Dependabot #4's drift fixed by formatting, with no lint weakened and no `// ignore`
- [ ] `just verify` green; six CI jobs green on the PR

## Out of scope

Merging the Dependabot PRs (the owner merges). `minSdk` (`T-008`). Any audio work (`T-002b`).

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._
