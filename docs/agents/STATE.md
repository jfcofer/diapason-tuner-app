# STATE — updated 2026-10-09 (T-002b part 2a)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> If you are an agent starting a session: run `just session-start`, read this, then the active task
> file, then begin. **PR and merge state changes between sessions**: this file records PRs by
> number only, and you reconcile it against what `session-start` prints before trusting it.

## Where we are

**Milestone:** M1 — Audio spine (`docs/ROADMAP.md`). **M0 closed 2026-10-09** (`T-006`); its one
unverifiable criterion, clean-clone setup on macOS, is recorded in the roadmap.
**Status:**

- **Merged:** T-006, T-002a, T-007, T-008, **T-002b part 1** and T-009 (PRs #6–#9, #11, #12).
- **Branch in flight:** `feat/T-002b-session` (T-002b part 2a), rebased on `main`, not pushed.
  `just verify` is green.
- The dev flavour runs on the Redmi. The tuner screen asks for the microphone and shows the live
  session.
- **`main` is protected** (ruleset `24468437`, no bypass): PR, six green checks, rebase-only.

## Active task

**`docs/agents/tasks/T-002b-android-duplex.md`**: part 1 is merged (#11). **Part 2a is built and
reviewed** on `feat/T-002b-session`; its notes record the review fixes and what 2b and part 3 owe.
- **Open before its PR:** `just test-integration-android` on the Redmi, with the owner at the
  device (HyperOS install and microphone prompts). The denied half passed on device. The granted
  half has not run.

## Hardware this project actually has

- A Fedora 44 host, and the **Redmi 23117RA68G** (Android 16, arm64): the "budget Android" row of
  `TESTING.md` §7. An emulator for permissions and lifecycle only, **never** for latency.
- **No Mac, no iPhone.** iOS is compiled and checked only by CI; from `T-002c`, on the Simulator.

## What exists now

- **Pub workspace:** 8 members. **Cargo:** `dsp`, `engine`, `audio_io`, `session`, `xtask`,
  `diapason_ffi`.
- **CI** (`docs/CI_RELEASE.md` §1): six jobs, every step a `just` recipe, toolchain from
  `tools/versions.env` via `.github/actions/toolchain`, SHA-pinned actions, weekly Dependabot.
- **The gate:** `just verify` = doctor-selftest (13/13), fmt, analyze, clippy, Dart + Rust tests and
  doctests, `deny`, `doc-rust`, codegen drift, `ios-project-check`, deps, docs, `lint-ci`.
- **iOS project** from `just ios-project`; **never edit `project.pbxproj` by hand.** Release checks:
  `check-android-release`, `check-ios-release`.
- **`AAudioBackend`** (`T-002b`): duplex as two AAudio streams on one clock, device-tested with
  `just test-android-device`. Android-only code is linted by `lint-rust-android`.
- **Session** (`T-002b` 2a, `adr/0022`): a sans-IO `Supervisor` on virtual time plus one thread;
  Dart sees `EngineHandle` (commands in, ~30 Hz `SessionSnapshot` out). `verifyEngineContract`
  binds fake and real engine; `just test-integration-android` runs it on a device.
- **Engine core** (`T-002a`): `Engine::prepare` → `(Engine, Processor)` over `rtrb` and
  `triple_buffer`; zero allocation proven by test with an armed canary; `engine/clippy.toml`.

## Decisions already made (do not re-litigate without an ADR)

The index is `docs/adr/README.md`. The ones a session most often runs into:
- `0001` Rust owns all audio, and Dart never sees a sample.
- `0011` pins live in `tools/versions.env`.
- `0015` Rust is pinned to an exact release.
- `0016` goldens run on the pinned ubuntu-24.04 runner.
- `0018` lossy casts are allowed only in `dsp::convert`, and `docs-check` rejects every other
  suppression.
- `0019` minSdk is 28, the first API with AAudio input presets.
- `0020` Android audio uses raw `ndk-sys` with our own wrapper, and `clock_gettime` is the one RT
  timing call.
- `0021` release builds abort on panic, and nothing catches it. The FFI `api` module denies
  `unwrap`, `expect`, `panic!` and unchecked indexing, and so must any new crate the FFI calls.
- `0022` the `session` crate owns the stream. Every (re)open replays the desired state, so every
  new `Command` needs a replay test.

## Traps a later session will otherwise re-discover

- **A clean checkout has no `*.g.dart`.** Run `just deps` before analyze, test or build. A local
  tree that already has them hides the problem, so test CI-shaped changes in a fresh clone.
- **`flutter build ipa` fails without a development team**, even `--no-codesign`: use
  `build-ios-unsigned`.
- **cargokit's vendored `build_tool` is not our code**, so it is excluded from format and analysis.
- **`doctor` uses GNU grep `-P`**, so not on macOS; `check-ios-release.sh` is bash 3.2-portable.
- **JDK 25 breaks Flutter Android builds** (flutter#187223). Flutter uses JDK 21 via `--jdk-dir`.
- **Compiling against a crate is not linking it.** Verify on the shipped binary.
- **A cold release build can exhaust this 14 GB host:** fat LTO for every ABI (cargokit ignores
  `--target-platform`). Build in the main checkout, never a worktree, with `CARGO_BUILD_JOBS=4`
  and nothing else running.
- **`cargo fmt` reformats FRB's generated file.** That is why formatting is part of `just gen`.
- **Editing a `clippy.toml` does not invalidate clippy's cache.** `touch` a source file before
  trusting a clean run. CI is unaffected.
- **HyperOS (the Redmi) refuses shell `pm grant`/`revoke` and `adb uninstall`; USB installs wait
  for a tap.** "USB debugging (Security settings)" would lift it.
- **`check-drift` diffs against the index and `ios-project-check` against HEAD:** stage (or
  commit) regenerated output before trusting either.

## Known gaps (not blocking)

- **CI reproducibility leftovers** are in `T-004`: unpinned tools (now including cargo-ndk) and
  Xcode, the AAB manifest, the `Podfile.lock` policy.
- **Goldens (M3):** `test-dart` will also run `core_ui` goldens locally. Tag them and exclude the tag
  from `test-dart` when the first golden lands (`adr/0016`).
- **The `audio_engine` plugin has no Swift Package Manager support.** Flutter 3.47 warns this will
  become an error. It needs cargokit SwiftPM support or a `Package.swift`, as its own task.
- **Rust crate licences are not in the in-app licence page** (`triple_buffer` MPL-2.0; M6).
- **`permission_handler` held at 12:** 13 needs compileSdk 37, its own task (`AGENTS.md` §8).
- **Nothing stops the stream yet:** the microphone stays open once the tuner listens (part 3).

## Open questions

| Question | Needed by | Notes |
|---|---|---|
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Reference devices beyond the Redmi | M1+ | No iPhone, Pixel or tablet available. `T-002` criteria were amended to "every device available" |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |
| Where `audio_io` puts lossy casts | `T-002c` | Android needed none (`try_from` throughout `android.rs`). Core Audio's `mSampleTime` (`f64`) does. Supersede `adr/0018` with a leaf crate, or let `audio_io` depend on `dsp` |
| Does the app get the AAudio fast path on the Redmi? | `T-002b` part 2b | The shell user is refused (`adr/0020`); a vendor per-app policy is suspected. Read `granted_paths` from the app |

## Next up (in order)

1. **Finish `T-002b` part 2a:** the device run, then PR, six green checks, rebase-merge (the owner
   approves). The owner also merges Dependabot #2 and #10.
2. **`T-002b` part 2b, then part 3** (see Active task).
3. **`T-002c`**, iOS CoreAudio, verified on the CI Simulator.
4. `T-004` (CI reproducibility) whenever a slice is waiting on CI. `T-003-pitch-core` after T-002.
