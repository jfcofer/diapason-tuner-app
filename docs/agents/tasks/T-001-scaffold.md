---
id: T-001
title: Scaffold the workspace, toolchain and CI
status: done
milestone: M0
owner: claude
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

- [x] Tree matches `docs/REPO_LAYOUT.md`; every package listed in the root `pubspec.yaml` workspace
- [x] `just doctor` verifies every version in `versions.env` and fails on any mismatch — and
      `just doctor-selftest` proves it, breaking each pin in a scratch copy (8/8 caught)
- [ ] `just setup` works from a clean clone on macOS and Linux — **Linux only. Closed
      unverifiable**: no macOS host. CI's `build-ios` runs the same toolchain and builds on macOS
- [x] `just verify` passes and runs: Dart format/analyze/test, Rust fmt/clippy -D warnings/
      nextest/doctests/deny, codegen-drift, dependency-direction, docs-check.
      (No `custom_lint` pass — `adr/0014` removed it)
- [x] `just run android` launches an app displaying a value returned from Rust across FFI
- [ ] `just run ios` — **closed unverifiable**: no macOS host. Replaced by CI `build-ios` (unsigned
      release build + `check-ios-release`) and, from `T-002c`, an iOS Simulator test in CI
- [x] dev/stg/prod flavours build and install side by side with distinct names and bundle IDs
- [x] `just rename <name> <bundle_id>` exists and validates its input
- [x] CI green — all six jobs, PR #1 (run 37230846035), 2026-10-04
- [x] Release Android build passes `just check-android-release` (targetSdk 36, 16 KB alignment)
- [x] `lefthook` hooks installed; a non-conventional commit message is rejected locally (verified
      both directions)
- [x] `.gitignore` covers keystores, `*.p8`, `*.p12`, `.env`, provisioning profiles, build outputs

## Out of scope

Any audio: no microphone, no stream, no DSP. Any real UI: no design tokens, no theme beyond
defaults. Any localisation. State management wiring beyond a single provider proving Riverpod is
configured. Signing and release automation (`T-0xx`, milestone M7).

## Implementation notes

**Deviations from the task as written**, each deliberate:

- **`custom_lint` removed** (`adr/0014`). It was unresolvable against Dart 3.13's analyzer, and
  modern `riverpod_lint` no longer uses it. `just lint` has one Dart pass, not two.
- **The FRB backend was re-decided** before scaffolding, by building both and measuring
  (`T-001a` → `adr/0012`). Cargokit confirmed.
- **Toolchain re-pinned** (`adr/0011`); pins moved out of prose and into `tools/versions.env`,
  which `doctor` and CI both read.
- **No Flutter l10n.** The task puts localisation out of scope; the locale-aware app *label* lives
  in native `strings.xml` / `InfoPlist.strings`, so no Dart l10n was needed to ship it.
- **`bench` is not in CI.** Out of scope here, and there is nothing to benchmark until `T-003`.
- **`i686-linux-android` added** to `rust-toolchain.toml`: cargokit builds it and rustup had been
  installing it implicitly, so a clean clone was not reproducible.

Four latent bugs surfaced by *running* the gate rather than trusting it — recorded because each
would have failed silently or misleadingly later:

1. `doctor`'s FRB check could not parse Cargo's exact-pin form `"=2.13.0"`, so it reported the
   dependency as missing.
2. `dart format .` walked into cargokit's vendored Dart `build_tool` under `apps/*/build/`.
3. `check-drift` diffed the whole working tree, so any uncommitted edit was reported as "generated
   code is out of date" with a fix that would not fix it.
4. The `lefthook` commit-msg hook read `$1` instead of lefthook's `{1}` placeholder, rejecting
   every message including valid ones.

**2026-10-04, getting CI green.** The first CI run had 5 of 7 jobs red, for five unrelated reasons.
Each was fixed at the root:

- **Vendored cargokit in the gate.** It is excluded from format and analysis.
- **Dev-only tools in `doctor`.** `doctor` now takes sections.
- **`sdkmanager` not on PATH** on ubuntu-24.04.
- **No flavour `--target`** in any build recipe.
- **A container that does not exist** for goldens (`adr/0016`).

The rewrite then surfaced three more problems:

- **Riverpod `*.g.dart` never generated** in clean checkouts. Fixed with `just deps`.
- **actionlint not packaged** by install-action. It is now pinned in `versions.env`.
- **`flutter build ipa` failing a team-less archive.** Fixed with `build-ios-unsigned`.

Further deviations:

- cargo-ndk was dropped, because nothing used it. melos runs from the lockfile.
- Rust is pinned to 1.98.0 (`adr/0015`).
- The iOS flavours are generated by `tools/ios/configure_project.rb`. The iOS dev/stg *launcher
  label* stays "Diapason" (`InfoPlist.strings` is per-locale, not per-flavour), but the bundle IDs
  differ, so the flavours still install side by side.

## Verification performed

Host: Fedora 44, Flutter 3.47.1 / Dart 3.13.1, Rust 1.98.0, JDK 21.0.12+1.1-tem, NDK r30, FRB 2.13.0.
Device: **Redmi 23117RA68G, HyperOS V816, Android 16 (API 36), arm64**.

- `just verify` green end to end: doctor-selftest (8/8), fmt-check, analyze `--fatal-infos` (0
  issues), 8/8 Dart packages, clippy `-D warnings`, nextest 2/2 + 1 doctest, cargo-deny, drift,
  deps, docs.
- **On the physical device:** the dev flavour renders `diapason_dsp 0.1.0` — a string computed in
  `rust/crates/dsp`, carried through `engine` → `ffi` → FRB → Dart. Screenshot taken.
- **Three flavours installed side by side:** `dev.jfcofer.diapason{,.dev,.stg}` all present in
  `pm list packages`.
- **Locale-aware label verified in the built artifact**, not just in source —
  `aapt2 dump resources` shows `() "Diapason (Dev)"` and `(es) "Diapasón (Dev)"`.
- **Release AAB** (48.7 MB, prod): targetSdk 36; every 64-bit `.so` at `LOAD align 0x4000`;
  46 MB against a 60 MB budget.
- **`check-deps` proven** by injecting all four violation classes and confirming each is caught.
- **`lefthook`** rejects a non-conventional message and accepts a conventional one.

**2026-10-04:**

- **CI.** All six jobs are green. `build-ios` produced `Runner.app` (16.3 MB), and
  `check-ios-release` passed: the privacy manifest is bundled, the mic string is set,
  MinimumOSVersion is 15.0, and the Rust engine is linked in `diapason_ffi.framework`.
- **Local.** `just verify` is green, with doctor-selftest catching 10/10. Release AAB checks pass:
  every 64-bit `.so` is at `0x4000` alignment, 46/60 MB.
- **Fresh clone.** The dart job sequence passes in a fresh clone.
- **Device.** The dev flavour runs on the Redmi and renders `diapason_dsp 0.1.0`.

**Not verified, and why (as of 2026-08-26; iOS compile is now covered by CI):**

- **Everything iOS.** No macOS host exists on this project. The iOS files are written — podspec at
  deployment target 15.0, `PrivacyInfo.xcprivacy` with tracking false, `NSMicrophoneUsageDescription`
  in both locales, background-audio mode, `en.lproj`/`es.lproj` `InfoPlist.strings` — but none of it
  has been compiled. The `build-ios` macOS CI job is the only thing that can check it.
- **Two iOS items need Xcode and are deliberately not hand-edited into `project.pbxproj`:**
  adding `PrivacyInfo.xcprivacy` to the Runner target's resources (it will not ship until this is
  done), and the per-flavour schemes/build configurations. Hand-writing unverifiable pbxproj UUIDs
  would be worse than leaving a clear note.
- **`just setup` on macOS.** Same reason.
