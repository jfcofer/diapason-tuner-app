# Development setup

This document owns **toolchain versions and the local loop**. Versions here are the contract; a
change to any of them is its own task and its own ADR (`AGENTS.md` §8).

## 1. Pinned versions

| Tool | Version | Pinned in |
|---|---|---|
| Flutter | 3.44.x stable (Dart 3.12) | `.fvmrc` |
| Rust | stable, edition 2024 | `rust-toolchain.toml` |
| flutter_rust_bridge | 2.12.x (codegen + runtime must match exactly) | `flutter_rust_bridge.yaml`, `Cargo.toml`, `pubspec.yaml` |
| Android NDK | r27+ (required for 16 KB page alignment) | `android/app/build.gradle.kts` |
| Android compile/target SDK | 36 · min 26 | same |
| Xcode | current App Store-accepted release | `.github/workflows/ci.yml` |
| iOS deployment target | 15.0 | `ios/Podfile`, project settings |
| Java | 21 (AGP requirement) | CI + `just doctor` |

Version drift between the FRB codegen binary and the runtime packages is the single most common
build failure in this stack. `just doctor` checks all three and refuses to proceed on a mismatch.

## 2. Prerequisites

```bash
# Flutter, pinned via fvm
dart pub global activate fvm && fvm install && fvm use

# Rust toolchain + mobile targets (rustup reads rust-toolchain.toml)
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android \
                  aarch64-apple-ios aarch64-apple-ios-sim

# Tooling
cargo install just cargo-nextest cargo-deny flutter_rust_bridge_codegen cargo-ndk
dart pub global activate melos
```

macOS is required to build and run the iOS target. Android and all Rust work run on any host.

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
| `Failed to lookup symbol` at startup | Native lib not rebuilt or not bundled | `just clean-native && just run` |
| FRB "generated code is out of date" | Codegen/runtime mismatch or stale bindings | `just doctor` then `just gen` |
| Android build fails after NDK bump | 16 KB alignment flags lost | Check `-Wl,-z,max-page-size=16384` reaches the Rust link step |
| iOS build: missing arch on simulator | `aarch64-apple-ios-sim` target absent | `rustup target add aarch64-apple-ios-sim` |
| Goldens fail only on your machine | Goldens are container-pinned | `just goldens-update`, never `--update-goldens` locally |
| Tuner reads wrong on one Android device | Input preset fell back | Check the preset field in the snapshot; see `PLATFORM_AUDIO.md` §2 |
| Metronome drifts | Something is using a wall clock | `AUDIO_ENGINE.md` §5 — the scheduling test should have caught it; add the case |

## 5. Conventions for local work

Never commit with hooks disabled. Never leave a `TODO` without a task ID (`// TODO(T-042):`).
Never check in `.env`, keystores, `*.p8`, `*.p12`, or provisioning profiles — `.gitignore` covers
them and `lefthook` scans for them, but the check is a safety net, not permission to be careless.
