# STATE — updated 2026-10-22 (T-003a built and verified on the host; PR next)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> If you are an agent starting a session: run `just session-start`, read this, then the active task
> file, then begin. **PR and merge state changes between sessions**: this file records PRs by
> number only, and you reconcile it against what `session-start` prints before trusting it.

## Where we are

**Milestone:** M1 — Audio spine (`docs/ROADMAP.md`). **M0 closed 2026-10-09** (`T-006`); its one
unverifiable criterion, clean-clone setup on macOS, is recorded in the roadmap. **Status:**

- **Merged:** T-006, T-002a, T-007–T-013, T-002b, `T-002d` part 1 (PRs #6–#9, #11–#16, #24,
  #27–#29), Dependabot #2, #10, #21, #22, #25, #26. #20 closed. **Open:** none.
- **`main` is protected** (ruleset `24468437`, no bypass): PR, six green checks, rebase-only.

## Active task

**`docs/agents/tasks/T-003a-mpm-core.md`** (in progress): `T-003`'s first slice, host-only M2 work
the owner chose on 2026-10-22, when no device was attached.
**Waiting on the Redmi:** `T-002d` (in progress; part 1 is #29): L3/L4, L10, `lifecycle_test.dart`.

## Hardware this project actually has

- Fedora 44 host and the **Redmi 23117RA68G** (Android 16, arm64), `TESTING.md` §7's budget
  Android. Emulator for lifecycle only, **never** latency. **No Mac, no iPhone:** iOS is CI-only.

## What exists now

- **Pub workspace:** 8 members. **Cargo:** `dsp`, `engine`, `audio_io`, `session`, `xtask`, `ffi`.
- **CI** (`docs/CI_RELEASE.md` §1): six jobs, every step a `just` recipe, toolchain from
  `tools/versions.env` via `.github/actions/toolchain`, SHA-pinned actions, weekly Dependabot.
- **The gate:** `just verify` = doctor-selftest, fmt (Dart, Rust, Kotlin), analyze, clippy, tests
  and doctests, `deny`, `doc-rust`, codegen drift, `ios-project-check`, deps, docs, `lint-ci`.
- **iOS project** from `just ios-project`; **never edit `project.pbxproj` by hand.**
- **`AAudioBackend`** (`T-002b`): duplex as two AAudio streams on one clock, device-tested with
  `just test-android-device`, linted by `lint-rust-android`. Shared, never MMAP, on the Redmi.
- **Session** (`T-002b` 2a, `adr/0022`): a sans-IO `Supervisor` on virtual time plus one thread;
  Dart sees `EngineHandle` (commands in, ~30 Hz `SessionSnapshot` out). `verifyEngineContract`
  binds fake and real engine; `just test-integration-android` runs it on a device, via patrol.
- **Tuner lifecycle** (`T-002d`): pure `engineTarget` policy, `AppLifecycle` seam, visibility
  from `TickerMode`. Matrix rows L1–L11 (`PLATFORM_AUDIO.md` §5); `docs-check` checks each proof.
- **Engine core** (`T-002a`): `Engine::prepare` → `(Engine, Processor)` over `rtrb` and
  `triple_buffer`; zero allocation proven by test with an armed canary; `engine/clippy.toml`.
- **Pitch core** (`T-003a`): `dsp::pitch::Mpm` (MPM over realfft, f64, allocation-free) and
  `dsp::synth` fixtures (`adr/0027`). `just bench` gates medians against a host-tagged baseline.

## Decisions already made (do not re-litigate without an ADR)

The index is `docs/adr/README.md`. The ones a session most often runs into:
- `0011` pins live in `tools/versions.env`.
- `0018` lossy casts only in `dsp::convert`; `docs-check` rejects every other suppression.
- `0019` minSdk is 28, the first API with AAudio input presets.
- `0020` Android audio uses raw `ndk-sys` with our own wrapper, and `clock_gettime` is the one RT
  timing call.
- `0021` release builds abort on panic, and nothing catches it. The FFI `api` module denies
  `unwrap`, `expect`, `panic!` and unchecked indexing, and so must any new crate the FFI calls.
- `0022` `session` owns the stream; every (re)open replays desired state: each `Command` needs a
  replay test.
- `0024` Pigeon for platform channels; Rust picks the preset; Dart tells the session the permission.
- `0025` buffer growth is session policy; only the start-up drain discards input.
- `0026` patrol runs device tests, CLI pinned in the lockfile, never in a release. It applies KGP,
  which a future Flutter refuses: check patrol before any Flutter bump.
- `0027` synthetic audio fixtures are generated in code; only recordings are checked in.

## Traps a later session will otherwise re-discover

- **A clean checkout has no `*.g.dart` or `*.g.kt`** (Pigeon). Run `just deps` first. A tree that
  has them hides this, so test CI-shaped changes in a fresh clone.
- **`flutter build ipa` needs a development team**, even `--no-codesign`: use `build-ios-unsigned`.
- **cargokit's vendored `build_tool` is not our code**, so it is excluded from format and analysis.
- **`doctor` uses GNU grep `-P`**, so not on macOS; `check-ios-release.sh` is bash 3.2-portable.
- **JDK 25 breaks Flutter Android builds** (flutter#187223). Flutter uses JDK 21 via `--jdk-dir`.
- **Compiling against a crate is not linking it.** Verify on the shipped binary.
- **A cold release build can exhaust this 14 GB host** (fat LTO for every ABI; cargokit ignores
  `--target-platform`): main checkout only, `CARGO_BUILD_JOBS=4`, nothing else running.
- **`cargo fmt` reformats FRB's generated file.** That is why formatting is part of `just gen`.
- **Editing a `clippy.toml` does not invalidate clippy's cache:** `touch` a source file first.
- **HyperOS (the Redmi) wants a tap on every USB install**, even with "USB debugging (Security
  settings)" on (it is). Unanswered: `INSTALL_FAILED_USER_RESTRICTED`, which the runner names.
- **Run patrol only through `tools/patrol.sh`:** bare, it edits `~/.bashrc`, hangs on Chrome,
  sends Google Analytics, and fails offline on a pub.dev update check.
- **`uiautomator dump` fails while the tuner listens** (30 Hz redraws): `rm` the old XML first,
  or use `screencap`. **A patrol run uninstalls the app**, and its APK is the test bundle.
- **`check-drift` diffs against the index and `ios-project-check` against HEAD:** stage (or
  commit) regenerated output before trusting either.
- **`dsp`'s clippy list is the RT list, tests included:** no `println!`, no `prop_assert!`.
  **`just bench` needs a quiet host:** agents running in parallel moved it by +20 %.

## Known gaps (not blocking)

- **CI reproducibility leftovers** are in `T-004`: unpinned cargo-ndk and Xcode, the AAB manifest,
  the `Podfile.lock` policy.
- **`audio_engine` lacks SwiftPM support**, which Flutter 3.47 warns will become an error: own task.
- **Rust crate licences are not in the in-app licence page** (`triple_buffer` MPL-2.0, realfft; M6).
- **`permission_handler` held at 12** (Dependabot too): 13 needs compileSdk 37, its own task.
- **`appLifecycleProvider` is in `feature_tuner`**; it moves down when the metronome needs it (M4).
- **A stall's input stays queued as latency** until a rebuild (`adr/0025`); none seen yet.
- **The Redmi now refuses the output the FAST path** (on `main` too), unlike part 1: `T-002f`.
- **`router.dart` uses plain `GoRoute`s**, not the typed routes `AGENTS.md` §7 requires.

## Open questions

| Question | Needed by | Notes |
|---|---|---|
| Who records the instrument fixtures | `T-003e` | Guitar, bass, ukulele, violin at known pitches, under a licence `adr/0009` permits |
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Reference devices beyond the Redmi | M1+ | No iPhone, Pixel or tablet available. `T-002` criteria were amended to "every device available" |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |
| Does opening or returning to the tuner start listening unasked? | M3 | `PRODUCT_SPEC.md` §1 and §2 say yes ("opens … listening"). `T-002d` and L11 say no, for privacy. M1 keeps "no". Owner decides, then an ADR |
| Where `audio_io` puts lossy casts | `T-002c` | Android needed none (`try_from` throughout `android.rs`). Core Audio's `mSampleTime` (`f64`) does. Supersede `adr/0018` with a leaf crate, or let `audio_io` depend on `dsp` |

## Next up (in order)

1. **`T-003a`**: PR, six green checks, owner rebase-merges. Then `T-003b` (its filter decides the
   sawtooth's ±0.5 ¢, see `T-003a` notes), `T-003c`, `T-003d`.
2. **When the Redmi is attached:** `T-002d` L3/L4 with a call, then L10 (the silenced microphone).
   Also owed: `T-003a`'s 4096 frame cost on the phone (≤ 1.5 ms).
3. **`T-002e`** (overlay in Settings → Diagnostics; first golden), then **`T-002f`** (latency, FAST).
4. **`T-002c`**, iOS CoreAudio, on the CI Simulator. `T-004` whenever a slice waits on CI.
