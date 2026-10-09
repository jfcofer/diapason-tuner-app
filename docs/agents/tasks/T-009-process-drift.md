---
id: T-009
title: Catch tasks left open after their PR merges; make the docs' panic claim true
status: done
milestone: M1
owner: claude
created: 2026-10-09
---

## Goal

Remove three kinds of drift that reconciling the repo on 2026-10-09 turned up, and make the first
one mechanically visible so it cannot come back unnoticed:

1. **A task stays `in-progress` after its work has merged.** `T-008` did.
2. **The docs promised `catch_unwind` at the FFI boundary.** Release builds abort on panic, so
   that promise was false.
3. **Several docs pointed at `rust/crates/ffi`,** which does not exist.

## Context

Read: `AGENTS.md` §2, §5 and §6, `tools/session-start.sh`, `docs/ARCHITECTURE.md` §7.
`T-007` built the session-start reconciliation this task extends.

Merged branches are usually deleted, by the owner or by GitHub. So a branch-based test
(`git cherry`) cannot see the case this task targets. PR head branches can, because they carry the
task ID (`AGENTS.md` §7).

## Acceptance criteria

- [x] `just session-start` prints every `in-progress` or `blocked` task with its PR state: open PR,
      last merged PR, or none. It stays informational, out of `just verify`, because the gate must
      not depend on remote state.
- [x] Setting `T-008` back to `in-progress` makes session-start name it with PR #9
- [x] `adr/0021` records the panic strategy. `ARCHITECTURE.md` §7 and `AGENTS.md` §6 cite it, and
      no doc mentions `catch_unwind` as a guarantee
- [x] The FFI `api` module denies the panicking constructs, and a deliberate `unwrap()` there
      fails clippy
- [x] No doc, script or agent command names `rust/crates/ffi` (journal and ADRs excepted: they
      are history)
- [x] `just verify` passes

## Out of scope

The stream supervisor crate, which adopts the same denials (`T-002b` part 2a). Changing the panic
strategy itself.

## Implementation notes

- **Multi-part tasks merge a PR per part.** A merged PR is therefore a prompt to check the task
  file, not proof that the task is done. The message says so, and `T-002b` shows the case today.
- **Merged PRs are sorted by `mergedAt` in `--jq`.** `gh pr list` orders by
  creation, which listed #7 above #6 even though #6 merged later. 30 PRs are fetched and 5 are shown.
- **The denials sit on `pub mod api`, not in `[lints]`.** flutter_rust_bridge's generated decoder
  in the same crate calls `unwrap()` 7 times (`adr/0021`).
- **Also fixed:** `README.md` still said "Oboe/AAudio", which `adr/0020` superseded.

## Verification performed

- `just session-start`: `T-002b in-progress  no open PR; last PR #11 merged 2026-10-09 …`.
- With `T-008` temporarily set to `in-progress`, it is listed with "last PR #9 merged". The file
  was restored afterwards.
- `shellcheck tools/session-start.sh`: clean.
- An `unwrap()` temporarily added to `api/simple.rs` failed clippy with `unwrap_used`. The file
  was restored afterwards.
- `just verify`: see the PR.
