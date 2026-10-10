---
id: T-010
title: Make the session checks catch what they claim to, and reconcile part 2a's merge
status: in-progress
milestone: M1
owner: claude
created: 2026-10-09
---

## Goal

Reconciling the repo after PR #13 found process checks that passed while checking nothing, and
docs that had drifted from the code and from GitHub. Fix each, prove each fix on a planted failure,
and record what part 2a's merge changed, so the next session starts from the truth.

## Context

Read: `AGENTS.md` §2, `tools/docs-check.sh`, `tools/session-end.sh`, `tools/session-start.sh`.
`T-007` and `T-009` built the session reconciliation this extends.

Found on 2026-10-09:
- The `docs-check` TODO scan had a space after `\K`, so it matched nothing: any `TODO(T-999)`
  passed.
- `session-end` compared against `HEAD~1`, which credited a session with the previous session's
  STATE edit. Its regex missed unstaged edits and every root file (`Cargo.toml`,
  `rust-toolchain.toml`, `justfile`). It never checked the task file, although `CONTEXT_SYSTEM.md`
  says it does.
- `T-002` stayed `todo` while `T-002a` was done and `T-002b` in progress, and still required an
  Oboe backend that `adr/0020` rejected. Nothing could see it: session-start skips `todo`, and no
  PR branch carries `/T-002-`.
- STATE.md said part 2a was not pushed; PR #13 had merged it.
- `CLAUDE.md` restated `settings.json` and had drifted from it (it said "read-only git").

## Acceptance criteria

- [x] A planted `TODO(T-999)` fails `docs-check`, tracked or untracked. The pattern proves itself
      on a known line, a `git grep` that cannot run fails, and `.kt`/`.swift` are scanned
- [x] `docs-check` fails a `todo` parent with a started slice, and a `done` parent with an open one
- [x] `session-start` names a `todo` task that already has a PR, and shows a parent's slices
- [x] `session-end` measures from the mark `session-start` leaves (else `origin/main`, with a
      note), counts unstaged, untracked, quoted and root paths, and fails when code changed but no
      task file did
- [x] `T-002` is in-progress with the AAudio amendment; `T-002b` records #13 and splits part 2b
- [x] `CLAUDE.md` points at `settings.json` instead of restating it
- [x] `just verify` passes

## Out of scope

Widening any permission. The `ask` rule for editing `build.gradle.kts` is the owner's to add: the
agent's edit to `settings.json` was refused by the permission classifier, which is the boundary
working, so `CLAUDE.md` says "ask first" instead.

## Implementation notes

- **Parent drift is checked offline, in `docs-check`.** A parent never has a PR of its own, so
  branch matching could not have caught `T-002`. Slice IDs share the parent's prefix, which can be
  tested with no network, so it fails in CI.
- **The session base is a file in the git dir** (`diapason-session-base`), written by
  `session-start`. The branch's merge-base was the first fix, but on a branch spanning sessions it
  still credits one session with another's work.
- **The `justfile` legend** now says a `[T-###]` tag names the task that introduced a recipe.
- **Review (`reviewer` subagent)** found the scan engine could fail silently, root files and
  quoted paths were invisible, a wrong `Disconnected` doc, and evidence missing for three ticks.
  All fixed. **Left open:** a self-test recipe that plants failures for `docs-check` and
  `session-end` the way `doctor-selftest` does. The probes below were run by hand.

## Verification performed

- `docs-check`: before `T-002` was fixed, two failures (one per started slice). `T-002` set to
  `done`: two failures (T-002b in progress, T-002c todo). `TODO(T-999)` in an untracked file: fails.
- `session-start` with `T-009` set to `todo`: "has PR #12 merged 2026-10-09, so it has started".
- `session-end`, in a throwaway local clone with a session mark:
  - an edit to the root `Cargo.toml` alone fails both the STATE check and the task-file check;
  - an untracked file named `apps/a b ñ.dart` also fails both;
  - code plus STATE plus any task file passes;
  - committed work with no mark falls back to `origin/main` and says so.
- `just verify`: green.
