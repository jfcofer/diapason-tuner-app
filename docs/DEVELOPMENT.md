# Development setup

This document owns **toolchain versions and the local loop**. Versions here are the contract; a
change to any of them is its own task and its own ADR (`AGENTS.md` §8).

## 1. Pinned versions

**`tools/versions.env` is the source of truth.** It is shell-sourceable, `just doctor` enforces it,
and CI reads the same file. This table explains *why* each pin is what it is; when the two disagree,
`versions.env` wins and this table is the bug. Changing a pin is its own task and its own ADR
(`AGENTS.md` §8).

| Tool | Version | Why this version |
|---|---|---|
| Flutter | 3.47.1 stable (Dart 3.13.1) | Current stable, 2026-08-19. Also the release whose iOS 15 / macOS 12 minimums we match |
| Rust | stable, edition 2024 | `rust-toolchain.toml` pins the channel, components and all five cross-compile targets |
| flutter_rust_bridge | 2.13.0 | Codegen binary, Rust crate and Dart package must match **exactly** — see below |
| Java | **21** (`21.0.12+1.1-tem`) | AGP is validated on 17 / partially 21. Flutter Android builds **fail on JDK 25** ([flutter#187223](https://github.com/flutter/flutter/issues/187223)) |
| Android NDK | r30 (`30.0.16138531`) | r28+ aligns shared libraries to 16 KB **by default**, so the old manual linker flag is no longer load-bearing |
| Android compile/target SDK | 36 · min 26 | targetSdk 36 required for Play submissions from 2026-08-31 |
| iOS deployment target | 15.0 | Flutter 3.47's own iOS minimum |
| Xcode | current App Store-accepted release | `.github/workflows/ci.yml`; iOS is built only on CI (see §2) |
| Melos | 8.x | Melos 7+ builds on pub workspaces and has no `melos.yaml` (`adr/0006`) |

Two failure modes are worth calling out because they cost hours and produce misleading errors:

**flutter_rust_bridge drift.** The codegen binary, the Rust crate and the Dart package are three
separate installs of the same version. Any two agreeing is not enough. `just doctor` checks all
three against `FRB_VERSION` and refuses to proceed on a mismatch.

**The JDK Gradle actually uses** is not necessarily the one on your `PATH`. Flutter prefers its own
`jdk-dir` setting, then Android Studio's bundled JBR, then `JAVA_HOME` — and Android Studio
currently bundles JBR 25, which is the version that breaks. `just doctor` resolves the JDK the same
way Flutter does and reports which source it came from.

## 2. Prerequisites

```bash
# Flutter, pinned via fvm (.fvmrc). A system Flutter already at the pinned version is fine —
# doctor checks the *active* version, not how you got it.
dart pub global activate fvm && fvm install && fvm use

# Java 21. Do NOT make it your global default if you need 25 elsewhere; point Flutter at it:
sdk install java 21.0.12+1.1-tem
flutter config --jdk-dir="$HOME/.sdkman/candidates/java/21.0.12+1.1-tem"

# Rust toolchain + mobile targets (rustup reads rust-toolchain.toml and installs them on first use)
rustup show

# Tooling. FRB is pinned deliberately — `cargo install` without --version installs latest and
# silently desynchronises you from the Rust and Dart sides.
cargo install cargo-nextest cargo-deny cargo-ndk --locked
cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked
dart pub global activate melos

# lefthook: binary release into a directory on your PATH (no package manager needed)
#   https://github.com/evilmartians/lefthook/releases

# `dart pub global activate` installs into ~/.pub-cache/bin, which is often not on PATH.
# Either add it, or symlink melos/fvm into a directory that is.
```

Then `just doctor`, which is the only authority on whether the above worked.

**macOS is required to build or run the iOS target.** Android and all Rust work run on any host.
On a Linux-only machine the `build-ios` CI job on a macOS runner is your sole iOS validation —
treat a red macOS job as a red build, not as someone else's problem.

## 3. The loop

```bash
just doctor     # verifies every version in §1 before you waste an hour
just setup      # workspace resolve + codegen + pre-commit hooks
just run ios    # or android — dev flavour
just verify     # what CI runs. Run it before you think you are done
```

Fast inner loops, for when `verify` is too slow to iterate against:

```bash
just test-rust dsp::pitch     # nextest filter
just test-dart feature_tuner  # one package
just bench pitch              # criterion, one group
just goldens-update           # regenerate goldens in the pinned container
```

Rust changes require a rebuild of the native library, so hot reload does not pick them up: `just run`
again. Dart-only changes hot reload normally. This asymmetry is the main friction of the stack —
keep engine iterations in `cargo test` where they are milliseconds, not in the app.

## 4. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Android build dies with "unsupported class version" or a native-access error | Gradle got JDK 25 | `just doctor` names the JDK and its source; `flutter config --jdk-dir=<jdk21>` |
| `Failed to lookup symbol` at startup | Native lib not rebuilt or not bundled | `just clean-native && just run` |
| FRB "generated code is out of date" | Codegen/runtime mismatch or stale bindings | `just doctor` then `just gen` |
| Android build fails after NDK bump | 16 KB alignment lost | NDK r28+ aligns by default; on r27 or below `-Wl,-z,max-page-size=16384` must reach the **Rust** link step, not just Gradle. Verify on the `.so`, never assume |
| iOS build: missing arch on simulator | `aarch64-apple-ios-sim` target absent | `rustup target add aarch64-apple-ios-sim` |
| Goldens fail only on your machine | Goldens are container-pinned | `just goldens-update`, never `--update-goldens` locally |
| Tuner reads wrong on one Android device | Input preset fell back | Check the preset field in the snapshot; see `PLATFORM_AUDIO.md` §2 |
| Metronome drifts | Something is using a wall clock | `AUDIO_ENGINE.md` §5 — the scheduling test should have caught it; add the case |

## 5. Conventions for local work

Never commit with hooks disabled. Never leave a `TODO` without a task ID (`// TODO(T-042):`).
Never check in `.env`, keystores, `*.p8`, `*.p12`, or provisioning profiles — `.gitignore` covers
them and `lefthook` scans for them, but the check is a safety net, not permission to be careless.
