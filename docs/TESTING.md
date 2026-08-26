# Testing strategy

This document owns **what each kind of change must prove**. The rule of thumb: the further from the
audio callback, the cheaper the test and the more of them there are; the closer to it, the more
deterministic the test must be.

## 1. What proves what

| If you change… | You must add or update… |
|---|---|
| `rust/crates/dsp` | Unit tests + a fixture-based accuracy assertion + a criterion bench |
| `rust/crates/engine` | Offline-backend scheduling/state tests, and the no-alloc test if the RT path changed |
| `rust/crates/audio_io` | A backend conformance test (the shared suite every backend must pass) |
| The FFI surface | A roundtrip test in Dart against the real engine, plus regenerate bindings |
| `core_domain` | Dart unit tests against `fixtures/note_table.json` |
| `core_ui` | Golden tests: 3 widths × 2 themes, plus a contrast assertion |
| `feature_*` | View-model unit tests against `FakeEngine`; widget tests for each declared state |
| Lifecycle / permissions | An integration test row in the matrix (`PLATFORM_AUDIO.md` §5) |

## 2. Rust

- Runner: `cargo nextest`. Lints: `clippy -D warnings`, `pedantic` on `dsp`.
- Supply chain: `cargo deny check` (licences, advisories, bans, duplicate versions) in CI.
- **Property tests** (`proptest`) for the pure functions where the invariant is clearer than any
  example: cents↔frequency roundtrips, temperament tables summing correctly, fixed-point phase
  accumulation never losing a beat over arbitrary tempo sequences.
- **Fixture accuracy tests**: every fixture in `fixtures/audio/` has an expected f₀ and an allowed
  error; the suite asserts the budgets in `AUDIO_ENGINE.md` §1. These are the tests that decide
  whether the app is good.
- **No-allocation test**: 10 s of audio through `OfflineBackend` with `assert_no_alloc` armed.
- **Scheduling test**: 30 minutes of offline audio, 47 tempos, every onset on its exact sample
  index. Zero tolerance — this test exists to make drift impossible to reintroduce.
- **Benches**: criterion, with baselines committed under `rust/benches/baselines/`.
  `just bench` fails a regression over 10 %.
- `miri` runs over `ffi` and `audio_io` unsafe blocks in a weekly CI job, not on every PR.

## 3. Dart

- Unit and widget tests with `flutter_test` + `mocktail`. Every view model is tested against
  `FakeEngine`, never the real one — UI tests must not need an audio device or a Rust build.
- **The fake is a contract, not a stub.** `FakeEngine` and the real engine are both driven through
  the same abstract interface, and a shared conformance suite runs against both (the real one only
  in the integration job). A fake that drifts from the real engine is worse than no fake.
- **Golden tests** are the primary defence for a heavily painted UI. Run them in a pinned Linux
  container only — font rendering differs by platform and goldens generated on a Mac will fail on
  CI forever. `just goldens-update` runs the same container locally.
- Integration tests with `patrol` for anything that touches a system dialog (microphone permission),
  the lock screen, or backgrounding.
- Accessibility assertions in widget tests: `meetsGuideline(textContrastGuideline)`,
  `androidTapTargetGuideline`, `iOSTapTargetGuideline`, `labeledTapTargetGuideline`.

## 4. What we do not test

Generated code. Third-party behaviour. Exact pixel values in logic tests (that is what goldens are
for). Wall-clock timing in unit tests — **ever**: if a test needs time to pass, it uses the offline
backend's virtual clock or `fakeAsync`.

## 5. Flakiness policy

A flaky test is a failing test. It gets fixed or deleted in the same session it is noticed, and the
decision is recorded in the journal. Never `skip` a test to get a green build; if you must, it is a
blocker in `STATE.md`.

## 6. Coverage

Targets, not gates: `dsp` and `core_domain` ≥ 90 %, `engine` ≥ 80 %, features ≥ 70 %, UI whatever
the goldens plus state tests naturally reach. Coverage is a smoke detector, not a goal — 100 % on a
component with no fixture accuracy test proves nothing.

## 7. Manual verification (before each release)

Reference devices, deliberately spanning the range where audio behaviour differs:

| Device | Why |
|---|---|
| A recent iPhone | ProMotion, the happy path |
| An iPhone SE-class device | Small screen, older audio hardware |
| A recent Pixel | AAudio exclusive mode, the Android happy path |
| A budget Android (< $200, 60 Hz) | Where the fast path is denied and the frame budget bites |
| One tablet (iPad + Android tablet, alternating) | Expanded layout |

Manual pass: the lifecycle matrix, tuning a real instrument against a known-good reference tuner,
30 minutes of metronome against a stopwatch, screen reader on both platforms, largest dynamic type,
and both languages.
