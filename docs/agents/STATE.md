# STATE — updated 2026-08-25 (bootstrap)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> If you are an agent starting a session: read this, then the active task file, then begin.

## Where we are

**Milestone:** M0 — Foundations (`docs/ROADMAP.md`)
**Status:** Not started. The repository contains architecture, specifications and the agent context
system. **No application code exists yet.**

## Active task

`docs/agents/tasks/T-001-scaffold.md` — create the workspace skeleton and get `just verify` green
on an empty app.

## Next up (in order)

1. `T-001-scaffold` — workspaces, toolchains, justfile, CI, empty app that builds on both platforms.
2. `T-002-audio-spine` — one duplex stream on both platforms through the real backends.
3. `T-003-pitch-core` — the `dsp` crate against fixtures, offline, no UI.

Do not start 2 before 1's exit criteria are objectively met. The ordering exists to retire risk
early, not to be tidy.

## Decisions already made (do not re-litigate without an ADR)

- Rust owns all audio; Dart never sees a sample. → `adr/0001`
- `AGENTS.md` is canonical; `CLAUDE.md` imports it. → `adr/0002`
- flutter_rust_bridge v2 + cargokit for the bridge and native build. → `adr/0003`
- Riverpod 3 (code-gen) for discrete state; painters driven by `Listenable` for continuous. → `adr/0004`
- MPM/NSDF for pitch detection. → `adr/0005`
- Pub workspaces + Melos for the monorepo. → `adr/0006`
- Note math intentionally duplicated in Rust and Dart, kept honest by a shared fixture. → `adr/0007`
- Procedural click synthesis, no audio assets in 1.0. → `adr/0008`

## Open questions (blocking nothing yet — answer before the milestone that needs it)

| Question | Needed by | Notes |
|---|---|---|
| Final app name and bundle ID | M0 end | `com.example.diapason` is a placeholder and cannot ship |
| Licence | M7 | Affects `cargo deny` config; see `adr/0009` |
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Which four reference devices are physically available | M1 | Budgets are meaningless without them |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |

## Blockers

None.

## Notes for the next agent

The repository is documentation-first on purpose. Read `AGENTS.md`, then `T-001`, and resist the
urge to start writing Dart before the workspace and CI exist — the first green `just verify` is what
makes every later session cheap.

`docs/BOOTSTRAP.md` explains how this repository was created and what the very first session should
do. Delete it once M0 is complete.
