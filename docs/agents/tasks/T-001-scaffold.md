---
id: T-001
title: Scaffold the workspace, toolchain and CI
status: todo
milestone: M0
owner: unassigned
created: 2026-08-25
---

## Goal

Turn this documentation-only repository into a buildable monorepo: pub workspace, Cargo workspace,
pinned toolchains, the `just` command surface, an empty-but-real Flutter app with three flavours,
a Rust FFI plugin that returns one value, and green CI on both platforms. Nothing after this is
cheap until this exists.

## Context

Read first: `docs/REPO_LAYOUT.md` (the exact tree to create), `docs/DEVELOPMENT.md` (pinned
versions), `docs/CI_RELEASE.md` §1–2 (jobs and flavours), `AGENTS.md` §4–5 (commands and
boundaries). You do not need the audio or design docs for this task.

Traps, in the order they will bite:
- flutter_rust_bridge codegen and runtime versions must match **exactly**; `just doctor` should fail
  loudly if they do not, and that check is more valuable than anything else in this task.
- cargokit expects the Rust crate inside the plugin package (`packages/audio_engine/rust`). Do not
  restructure it; make it a member of the root Cargo workspace instead.
- The Android 16 KB page-alignment linker flag must reach the Rust link step, not just the Gradle
  one — verify on the produced `.so`, do not assume.
- iOS needs both `aarch64-apple-ios` and `aarch64-apple-ios-sim`, and the simulator target is the
  one people forget until the first simulator run fails.

## Acceptance criteria

- [ ] Tree matches `docs/REPO_LAYOUT.md`; every package listed in the root `pubspec.yaml` workspace
- [ ] `just doctor` verifies every version in `DEVELOPMENT.md` §1 and fails on any mismatch
- [ ] `just setup` works from a clean clone on macOS and Linux
- [ ] `just verify` passes and runs: Dart format/analyze/custom_lint/test, Rust
      fmt/clippy -D warnings/nextest/deny, codegen-drift check, dependency-direction check,
      docs-check
- [ ] `just run android` and `just run ios` launch an app that displays one value returned from Rust
      through the FFI boundary
- [ ] dev/stg/prod flavours build and install side by side with distinct names and bundle IDs
- [ ] `just rename <name> <bundle_id>` renames packages, crates, bundle IDs and display names, and
      the repo still builds afterwards
- [ ] CI green: every blocking job in `CI_RELEASE.md` §1 except `bench` and `integration`
- [ ] Release Android build passes `just check-android-release` (targetSdk 36, 16 KB alignment)
- [ ] `lefthook` hooks installed by `just setup`; a commit with bad formatting is rejected locally
- [ ] `.gitignore` covers keystores, `*.p8`, `*.p12`, `.env`, provisioning profiles, build outputs

## Out of scope

Any audio: no microphone, no stream, no DSP. Any real UI: no design tokens, no theme beyond
defaults. Any localisation. State management wiring beyond a single provider proving Riverpod is
configured. Signing and release automation (`T-0xx`, milestone M7).

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._
