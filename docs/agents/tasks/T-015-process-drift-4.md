---
id: T-015
title: Close the session-end loop for every agent and catch stale code names in living docs
status: in-progress
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

Reconciling the repo on 2026-10-30 found process drift that no check caught:
- Two working days of `T-003c` review (2026-10-24 and 10-25, about 15 commits) left no journal
  entry. The cause: `/session-end` runs `just verify` but never `just session-end`, the recipe that
  checks the journal, and AGENTS.md §2 does not name that recipe for other agents.
- Commit d009555 removed `NEW_NOTE_CLARITY` while `AUDIO_ENGINE.md` and ADR 0029 still described it.
- Five known gaps in STATE.md say "own task" and have none.

This task makes each of those failures fail a check, or gives the gap an owner.

## Context

Read first: `AGENTS.md` §2; `docs/agents/CONTEXT_SYSTEM.md`; `tools/docs-check.sh` (how a check proves
its pattern on a known line first, `T-010`); `tools/session-end.sh`; `docs/adr/README.md`;
`docs/agents/tasks/README.md`.

Prior art: `T-009`, `T-010` and `T-012` (process drift 1–3). Known trap: a pattern that matches
nothing passes silently.

## Acceptance criteria

- [x] **Session end, for every agent:** AGENTS.md §2's end list ends with `just session-end`, and
  `.claude/commands/session-end.md` runs it (it already runs `verify`). A branch with no journal
  entry fails it.
- [x] **Stale code names:** `docs-check` fails when a backticked `SCREAMING_CASE` constant or
  `Type::member` path in `docs/*.md` or STATE.md exists in no tracked file outside `docs/`.
  - ADRs and the journal are historical and skipped.
  - The pattern proves itself on a known line.
  - A scan error fails the check.
  - A planted `` `NOT_A_REAL_CONST` `` fails it.
  - False positives on today's tree are measured first, and the scope is narrowed rather than
    allowlisted.
- [x] **ADR rules:** `adr/README.md` says that an ADR is immutable once on `main` and may be revised
  on its branch before merge, noted in its status line. "Accepted · in part superseded by NNNN" is
  a documented status, and `docs-check`'s status pattern matches the documented list exactly.
- [x] **Agent-agnostic claims:** AGENTS.md:4 and CONTEXT_SYSTEM.md:44 say only what each tool's current
  docs confirm about reading `AGENTS.md`. Any config pointer (Gemini CLI `context.fileName`, Aider
  `read:`) is added only with a source cited in the commit.
- [x] **Every known gap has one owner:**
  - new `todo` task files for typed `go_router` routes (AGENTS.md §7), SwiftPM for `audio_engine`,
    and `permission_handler` 13 (compileSdk 37, its own ADR);
  - ROADMAP M6 names the Rust crate licences, and M4 the `appLifecycleProvider` move;
  - the stall-latency gap (`adr/0025`) gets an owner.
- [x] **Honest states:** T-002d is `blocked` on the Redmi, and `tasks/README.md` allows one active
  task per agent, with blocked tasks naming what unblocks them.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

Doing the tasks the gaps get (typed routes, SwiftPM, `permission_handler` 13). Per-tool instruction
files beyond a pointer to `AGENTS.md` (`CONTEXT_SYSTEM.md` §"one file").

## Implementation notes

- **Session end:** AGENTS.md §2 step 5 and `/session-end` step 7 run `just session-end`, which
  already ran `verify` after its own checks; nothing else needed it.
- **Code names:** a `docs-check` section. Before scoping, today's tree had two hits, both Android
  constants in `PLATFORM_AUDIO.md` (`POST_NOTIFICATIONS`, `USAGE_MEDIA`): that file is skipped as a
  platform reference, with no allowlist. `T-003c`'s branch scans clean too.
- **ADR status:** documented as `<status> · YYYY-MM-DD` plus an optional partial supersession or
  note; the pattern now requires the date and rejects `Accepted` alone or unknown words.
- **Agents:** Gemini CLI defaults to `GEMINI.md` (geminicli.com/docs/cli/gemini-md,
  `context.fileName`); Aider loads only `read:` files (aider.chat/docs/usage/conventions.html).
  So `.gemini/settings.json` and `.aider.conf.yml` point at `AGENTS.md`; CONTEXT_SYSTEM says so.
- **Owners:** `T-016` typed routes, `T-017` SwiftPM, `T-018` permission_handler 13 (Dependabot's
  comment names it); ROADMAP M4 and M6; `T-002f` gets the stall-latency criterion.
- **Blocked:** T-002d is `blocked`, with a `**Blocked on**` line, which `docs-check` now requires of
  every blocked task.

## Verification performed

2026-10-30, host. `docs-check` fails on a planted `` `NOT_A_REAL_CONST` `` and `Observation::Faint`
in `AUDIO_ENGINE.md`, and on a blocked task without its line; the ADR status pattern was run on six
good and bad lines. `tools/session-end.sh` failed on this branch before its journal entry existed.
