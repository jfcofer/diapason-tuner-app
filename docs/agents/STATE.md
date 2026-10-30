# STATE — updated 2026-10-30 (T-020 merged: no wrong note shown as certain on recordings; T-003f next)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> Agents: run `just session-start`, read this, then the active task file. PR and merge state
> changes between sessions: reconcile the PR numbers here against what `session-start` prints.

## Where we are

**Milestone:** M1 — Audio spine (`docs/ROADMAP.md`). **M0 closed 2026-10-09** (`T-006`); its one
unverifiable criterion, clean-clone setup on macOS, is recorded in the roadmap. **Status:**

- **Merged:** T-006, T-002a, T-007–T-013, T-002b, `T-002d` part 1, `T-003a`–`T-003c`, T-015,
  T-019, `T-003e` and `T-020` with this snapshot (PRs #6–#9, #11–#16, #24, #27–#31, #34, #36,
  #38, #39, #40), Dependabot #2, #10, #21, #22, #25, #26. #20, #33 closed; #35 closed, superseded
  by #36. **Open:** Dependabot #32 (actions) and #37 (rust-minor), green, for the owner to merge.
- **`main` is protected** (ruleset `24468437`, no bypass): PR, six green checks, rebase-only.

## Active task

**`docs/agents/tasks/T-003f-smoothing.md`** (todo): median of five, one-euro, lock, the 260 ms
gate. `T-020` (`adr/0031`) owes one `just bench` on a quiet host; none was possible (its file).
**Waiting on the Redmi:** `T-002d` (blocked; part 1 is #29): L3/L4, L10, `lifecycle_test.dart`.

## Hardware this project actually has

- Fedora 44 host; **Redmi 23117RA68G** (Android 16, arm64, `TESTING.md` §7's budget Android).
  Emulator for lifecycle only, **never** latency. **No Mac, no iPhone:** iOS is CI-only.

## What exists now

- **Pub workspace:** 8 members. **Cargo:** `dsp`, `engine`, `audio_io`, `session`, `xtask`, `ffi`.
- **CI** (`docs/CI_RELEASE.md` §1): six jobs, every step a `just` recipe, toolchain from
  `tools/versions.env` via `.github/actions/toolchain`, SHA-pinned actions, weekly Dependabot.
- **The gate:** `just verify` (its recipe lists the steps). **iOS project** from `just ios-project`;
  never edit `project.pbxproj` by hand.
- **`AAudioBackend`** (`T-002b`): duplex as two AAudio streams on one clock, device-tested with
  `just test-android-device`, linted by `lint-rust-android`. Shared, never MMAP, on the Redmi.
- **Session** (`T-002b` 2a, `adr/0022`): a sans-IO `Supervisor` on virtual time plus one thread;
  Dart sees `EngineHandle` (commands in, ~30 Hz `SessionSnapshot` out). `verifyEngineContract`
  binds fake and real engine; `just test-integration-android` runs it on a device, via patrol.
- **Tuner lifecycle** (`T-002d`): `engineTarget` policy, `AppLifecycle` seam; rows L1–L11 checked.
- **Engine core** (`T-002a`): `Engine::prepare` → `(Engine, Processor)` over `rtrb`/`triple_buffer`.
- **Pitch** (`T-003a`–`c`): `Mpm`, `synth` (`adr/0027`), `chain::Front` (RT) and `chain::Analysis`
  split at the ring (`adr/0028`); octave guard, verifiers (`adr/0029`, `0031`). ±0.09 ¢.
- **Recorded fixtures** (`T-003e`, `adr/0030`): `fixtures/audio/`, `cargo xtask fixtures
  import|reference|check` (`check` in `verify`), `recorded.rs`: no gross frame on gold or ceiling.

## Decisions already made (do not re-litigate without an ADR)

The index is `docs/adr/README.md`. The ones a session most often runs into:
- `0011` pins live in `tools/versions.env`.
- `0018` lossy casts only in `dsp::convert`; `docs-check` rejects every other suppression.
- `0019` minSdk is 28, the first API with AAudio input presets.
- `0020` Android audio: raw `ndk-sys`, our own wrapper; `clock_gettime` is the one RT timing call.
- `0021` release builds abort on panic, and nothing catches it. The FFI `api` module denies
  `unwrap`, `expect`, `panic!` and unchecked indexing, and so must any new crate the FFI calls.
- `0022` `session` owns the stream; every (re)open replays desired state (a replay test each).
- `0024` Pigeon for platform channels; Rust picks the preset; Dart tells the session the permission.
- `0025` buffer growth is session policy; only the start-up drain discards input.
- `0028` windows, hops, crossover from a worst-case latency ledger; **tuner snapshots at 60 Hz**;
  budgets ≥ 160 Hz ≤ 85 ms, 80–160 Hz ≤ 110 ms, low B ≤ 190 ms (owner, 2026-10-23).
- `0026` patrol runs device tests, never in a release; it applies KGP: check before Flutter bumps.
- `0027` synthetic audio fixtures are generated in code; only recordings are checked in.
- `0031` a verifier moves the chain down only to a pitch the current range cannot see.

## Traps a later session will otherwise re-discover

- **A clean checkout has no `*.g.dart` or `*.g.kt`** (Pigeon). Run `just deps` first. A tree that
  has them hides this, so test CI-shaped changes in a fresh clone.
- **`flutter build ipa` needs a development team**, even `--no-codesign`: use `build-ios-unsigned`.
- **`doctor` uses GNU grep `-P`**, so not on macOS; `check-ios-release.sh` is bash 3.2-portable.
- **JDK 25 breaks Flutter Android builds** (flutter#187223). Flutter uses JDK 21 via `--jdk-dir`.
- **Compiling against a crate is not linking it.** Verify on the shipped binary.
- **A cold release build can exhaust this 14 GB host** (fat LTO for every ABI; cargokit ignores
  `--target-platform`): main checkout only, `CARGO_BUILD_JOBS=4`, nothing else running.
- **`cargo fmt` reformats FRB's generated file**, so `just gen` formats. cargokit is not our code.
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
  **`just bench` needs a quiet host** (parallel agents: +20 %); even idle, 2 of 4 runs failed one
  µs-scale entry at 10 %. `dsp` tests take ~50 s in a debug build.

## Known gaps (not blocking)

- **CI reproducibility leftovers** (`T-004`): unpinned cargo-ndk, Xcode; AAB manifest; `Podfile.lock`.
- **`audio_engine` lacks SwiftPM support** (Flutter 3.47 warns it will be an error): `T-017`.
- **`permission_handler` held at 12** (Dependabot too): 13 needs compileSdk 37, `T-018`.
- **Rust crate licences** missing from the licence page (M6), **`appLifecycleProvider`** moving down
  (M4): both in `ROADMAP.md`. **A stall's input queued as latency** (`adr/0025`): `T-002f`.
- **The Redmi now refuses the output the FAST path** (on `main` too), unlike part 1: `T-002f`.
- **`router.dart` uses plain `GoRoute`s**, not the typed routes `AGENTS.md` §7 requires: `T-016`.

## Open questions

| Question | Needed by | Notes |
|---|---|---|
| Recorded budgets for bass, ukulele, violin, electric guitar | `T-003g` | §1 budgets only acoustic guitar. Measured under ceilings in `recorded.rs` (every median within 2.6 ¢ of the consensus); budgets by ADR |
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Reference devices beyond the Redmi | M1+ | No iPhone, Pixel or tablet available. `T-002` criteria were amended to "every device available" |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |
| Does opening or returning to the tuner start listening unasked? | M3 | `PRODUCT_SPEC.md` §1 and §2 say yes ("opens … listening"). `T-002d` and L11 say no, for privacy. M1 keeps "no". Owner decides, then an ADR |
| Rates other than 44.1/48 kHz | M3 | `RangeTable::new` rejects them; `PLATFORM_AUDIO.md` says the granted rate is used as is |
| Where `audio_io` puts lossy casts | `T-002c` | Android needed none (`try_from` throughout `android.rs`). Core Audio's `mSampleTime` (`f64`) does. Supersede `adr/0018` with a leaf crate, or let `audio_io` depend on `dsp` |

## Next up (in order)

1. **`T-003f`** (smoothing and lock), then `T-014` (chords; `T-020` added the G4-over-G4/3 case),
   `T-003d`, `T-003g`. The owner may add the `SessionStart` hook `T-019` left out (its notes).
2. **When the Redmi is attached:** `T-002d` L3/L4 with a call, then L10 (the silenced microphone).
   Also owed: `T-003a`'s 4096 frame cost on the phone (≤ 1.5 ms).
3. **`T-002e`** (overlay in Settings → Diagnostics; first golden), then **`T-002f`** (latency, FAST).
4. **`T-002c`**, iOS CoreAudio, on the CI Simulator. `T-004` whenever a slice waits on CI.
