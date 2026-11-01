# STATE — updated 2026-11-01 (T-022: names a branch never pushed; T-025 next)

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> Agents: run `just session-start`, read this, then the active task file. PR and merge state
> changes between sessions: reconcile the PR numbers here against what `session-start` prints.

## Where we are

**The app cannot show a note yet:** nothing outside its own tests calls `dsp::chain`. By the owner's
decision (2026-11-01, `T-024`) the next product work is the vertical slice `T-026`–`T-032`, which crosses
M1, M2 and M3 (`docs/ROADMAP.md`) before they close. **1.0 is Android only** (`adr/0035`). **Status:**

- **Merged:** T-006, T-002a, T-007–T-013, T-002b, `T-002d` part 1, `T-003a`–`T-003c`, T-015,
  T-019, `T-003e`, `T-020`, `T-003f`, `T-014`, `T-021`, `T-024`, `T-022` (PRs #6–#9, #11–#16, #24,
  #27–#31, #34, #36, #38–#45, and this one), Dependabot #2, #10, #21, #22, #25, #26,
  #32, #37. #20, #33 closed; #35 closed, superseded by #36. **Open:** none.
- **`main` is protected** (ruleset `24468437`, no bypass): PR, six green checks, rebase-only.
- **Hardware:** Fedora 44 host; **Redmi 23117RA68G** (Android 16, arm64), the one reference device.
  Emulator for lifecycle only, **never** latency. No Mac, no iPhone: iOS is post-1.0 (`adr/0035`).

## Active task

**`docs/agents/tasks/T-025-docs-truth-and-portability.md`** (todo): true docs, checked by `docs-check`; a
smaller STATE; rules for every agent in `AGENTS.md`. Then `T-003d` and `T-027`.
**Waiting on the Redmi:** `T-026` (the kill tests, with the owner playing) and `T-002d` (L3/L4, L10).

## What exists now

- **Pub workspace:** 8 members. **Cargo:** `dsp`, `engine`, `audio_io`, `session`, `xtask`, `ffi`.
- **CI** (`docs/CI_RELEASE.md` §1): six jobs, every step a `just` recipe, toolchain from
  `tools/versions.env`, SHA-pinned actions, weekly Dependabot. **The gate:** `just verify`. **iOS
  project** from `just ios-project`; never edit `project.pbxproj` by hand.
- **`AAudioBackend`** (`T-002b`): duplex as two AAudio streams on one clock, device-tested with
  `just test-android-device`, linted by `lint-rust-android`. Shared, never MMAP, on the Redmi.
- **Session** (`adr/0022`): a sans-IO `Supervisor` plus one thread; Dart sees `EngineHandle`.
  `verifyEngineContract` binds fake and real engine; on a device: `just test-integration-android`.
- **Tuner lifecycle** (`T-002d`): `engineTarget` policy, `AppLifecycle` seam. Matrix status: its §5 table.
- **Engine core** (`T-002a`): `Engine::prepare` → `(Engine, Processor)` over `rtrb`/`triple_buffer`.
- **Pitch** (`T-003a`–`c`, `f`, `T-014`, `T-021`): `Mpm`, `chain::Front` (RT) / `chain::Analysis`
  (`adr/0028`); octave guard, verifiers (`0029`, `0031`); ±0.09 ¢. Needle and lock (`0032`,
  `Frame::display`, `dsp::lock`), gated on §1's latency rows. Chords (`0033`, `0034`): `Frame::held`.
  **Recordings** (`adr/0030`): `cargo xtask fixtures import|reference|check`, `recorded.rs`.

## Decisions already made (do not re-litigate without an ADR)

The index is `docs/adr/README.md`. The ones a session most often runs into:
- `0011` pins live in `tools/versions.env`. `0019` minSdk is 28, the first API with AAudio presets.
- `0018` lossy casts only in `dsp::convert`; `docs-check` rejects every other suppression.
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
- `0031` a verifier moves down only to a third or a quarter of a pitch the range cannot see.
  `0032` the needle: one-euro in cents, ≤ 3 ¢ behind the median; lock takes the caller's cents.
- `0033`/`0034` a chord is told from a string by the partials it sounds, never an onset.
- `0035` Android only for 1.0. `0036` the tuner listens by itself after one consent, only while visible.

## Traps a later session will otherwise re-discover

- **A clean checkout has no `*.g.dart` or `*.g.kt`** (Pigeon). Run `just deps` first. A tree that
  has them hides this, so test CI-shaped changes in a fresh clone.
- **`doctor` uses GNU grep `-P`** (not macOS). **`flutter build ipa` needs a team**: `build-ios-unsigned`.
  **JDK 25 breaks Flutter Android** (flutter#187223): Flutter uses JDK 21 via `--jdk-dir`.
- **A cold release build can exhaust this 14 GB host** (fat LTO for every ABI; cargokit ignores
  `--target-platform`): main checkout only, `CARGO_BUILD_JOBS=4`, nothing else running.
- **`cargo fmt` reformats FRB's generated file**, so `just gen` formats (cargokit is not ours).
  **A `clippy.toml` edit keeps clippy's cache:** `touch` a source file. **Compiling a crate is not
  linking it:** verify on the shipped binary.
- **HyperOS (the Redmi) wants a tap on every USB install**, even with "USB debugging (Security
  settings)" on (it is). Unanswered: `INSTALL_FAILED_USER_RESTRICTED`, which the runner names.
- **Run patrol only through `tools/patrol.sh`:** bare, it edits `~/.bashrc`, hangs on Chrome,
  sends Google Analytics, and fails offline on a pub.dev update check.
- **`uiautomator dump` fails while the tuner listens** (30 Hz redraws): `rm` the old XML first,
  or use `screencap`. **A patrol run uninstalls the app**, and its APK is the test bundle.
- **`check-drift` diffs the index, `ios-project-check` HEAD:** stage regenerated output first.
  **`dsp`'s clippy list is the RT list, tests too:** no `println!`, `prop_assert!`, `Vec::push`.
  **`just bench` needs a quiet host** (parallel agents: +20 %, just after `verify`: +68 %); even
  idle, 2 of 4 runs failed one µs-scale entry at 10 %. `dsp` tests take ~50 s in a debug build.

## Known gaps (not blocking)

- **CI leftovers** (`T-004`): unpinned cargo-ndk, Xcode; AAB manifest; `Podfile.lock`. **`T-017`**
  (SwiftPM) is blocked by `adr/0035`, which also says what happens if a Flutter bump forces it.
  **`permission_handler` at 12** (13 needs compileSdk 37): `T-018`.
- **Rust crate licences** (M6), **`appLifecycleProvider`** moving down (M4): in `ROADMAP.md`.
  `T-002f`: **a stall's input queued as latency** (`adr/0025`); the Redmi refuses output FAST.
- **Weak odd partials, 107–150 Hz:** 2f₀ shown known ~60 ms before Mid corrects it (`adr/0031`).
  **Chords:** fifths and unequal strums from silence show a third note (`T-023`, blocked on `T-026`);
  ceilings in `adr/0033` and `0034`, all accepted by the owner.
- **A2 → low B, 12 dB under, 44.1 kHz:** 1.9 ms past its 190 ms target (§1). **No bench baseline
  since `T-020`:** `chain/smooth`, `chain/chord`, `chain/strum` reported, not gated (Next up).
- **`router.dart` uses plain `GoRoute`s**, not the typed routes `AGENTS.md` §7 requires: `T-016`.

## Open questions

| Question | Needed by | Notes |
|---|---|---|
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |
| While two strings ring loud, hold the old note until it decays (> 1.5 s), or show nothing? | M3 | `adr/0033` holds, as `PRODUCT_SPEC.md` §2 says. A phone mic's high-pass may also hold a real low string under its harmonic; re-measure on the Redmi |
| A second Android OEM before release? | M7 | `adr/0035`: one phone is all the evidence until M7's closed test. A Samsung or a Pixel would show input-preset and latency differences early |
| Rates other than 44.1/48 kHz | `T-027` | `RangeTable::new` rejects them. `T-027` asks AAudio for 48 kHz so it resamples; any other rate reports the tuner unavailable |

## Next up (in order)

1. **`T-025`** (true docs, portable protocol, a smaller STATE).
2. **`T-026`, with the owner and the Redmi:** frame cost on the phone and real microphone captures. Its
   gate can change everything after it, and it decides `T-023` and `T-003g`.
3. **`T-003d`**, then the slice: **`T-027` → `T-028` → `T-029` → `T-030`**, host-testable without the
   Redmi, then **`T-031`** (a real note, measured latency) and **`T-032`** (a stall on a real string).
4. **`T-033`** (the strobe display), which also holds `T-016`, `T-002e` and `T-002f`. `T-002d`'s rows
   whenever the Redmi is attached.
5. **To close M2:** `T-003h`; and, on a quiet host, the owner's `just bench`, then `just bench-baseline`
   (it adopts `chain/smooth`, `chain/chord`, `chain/strum`; `T-019`'s hook). `T-004` as slices need it.
