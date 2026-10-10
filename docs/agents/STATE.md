# STATE — updated 2026-10-09 (T-002b part 2b-i)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> If you are an agent starting a session: run `just session-start`, read this, then the active task
> file, then begin. **PR and merge state changes between sessions**: this file records PRs by
> number only, and you reconcile it against what `session-start` prints before trusting it.

## Where we are

**Milestone:** M1 — Audio spine (`docs/ROADMAP.md`). **M0 closed 2026-10-09** (`T-006`); its one
unverifiable criterion, clean-clone setup on macOS, is recorded in the roadmap. **Status:**

- **Merged:** T-006, T-002a, T-007, T-008, T-002b parts 1 and 2a, T-009, T-010, T-011 (PRs
  #6–#9, #11–#15), and Dependabot #2 and #10.
- **Branch in flight:** `feat/T-002b-capabilities` (2b-i), passed on the Redmi; its PR is next.
- The dev flavour runs on the Redmi. The tuner screen asks for the microphone and shows the live
  session.
- **`main` is protected** (ruleset `24468437`, no bypass): PR, six green checks, rebase-only.

## Active task

**`docs/agents/tasks/T-002b-android-duplex.md`**: parts 1 and 2a merged (#11, #13). **2b-i is
built and device-tested** (`adr/0024`): capabilities over Pigeon, the preset rule in Rust, the
permission told to the session; VoiceRecognition is proven applied on the Redmi. Then 2b-ii
(stream tuning).

## Hardware this project actually has

- A Fedora 44 host, and the **Redmi 23117RA68G** (Android 16, arm64): the "budget Android" row of
  `TESTING.md` §7. An emulator for permissions and lifecycle only, **never** for latency.
- **No Mac, no iPhone.** iOS is compiled and checked only by CI; from `T-002c`, on the Simulator.

## What exists now

- **Pub workspace:** 8 members. **Cargo:** `dsp`, `engine`, `audio_io`, `session`, `xtask`,
  `diapason_ffi`.
- **CI** (`docs/CI_RELEASE.md` §1): six jobs, every step a `just` recipe, toolchain from
  `tools/versions.env` via `.github/actions/toolchain`, SHA-pinned actions, weekly Dependabot.
- **The gate:** `just verify` = doctor-selftest (15/15), fmt (Dart, Rust, Kotlin), analyze,
  clippy, Dart + Rust tests and doctests, `deny`, `doc-rust`, codegen drift, `ios-project-check`,
  deps, docs, `lint-ci`.
- **iOS project** from `just ios-project`; **never edit `project.pbxproj` by hand.** Release checks:
  `check-android-release`, `check-ios-release`.
- **`AAudioBackend`** (`T-002b`): duplex as two AAudio streams on one clock, device-tested with
  `just test-android-device`, linted by `lint-rust-android`. On the Redmi the app gets the
  low-latency (FAST) path, shared not MMAP; only the shell user is refused.
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
- `0023` ktfmt formats the Kotlin we write (not `*.g.kt`, not Gradle `*.kts`), in `verify`.
- `0024` Pigeon for platform channels; Rust picks the preset; Dart tells the session the permission.

## Traps a later session will otherwise re-discover

- **A clean checkout has no `*.g.dart` or `*.g.kt`** (Pigeon). Run `just deps` first. A tree that
  has them hides this, so test CI-shaped changes in a fresh clone.
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

## Next up (in order)

1. **The PR for 2b-i** (push when the owner says so).
2. **`T-002b` 2b-ii**, then **part 3** (overlay, lifecycle, latency).
3. **`T-002c`**, iOS CoreAudio, verified on the CI Simulator.
4. `T-004` whenever a slice is waiting on CI. **`T-003-pitch-core` can run in parallel** with M1:
   it is host-only `dsp` work with no device dependency.
