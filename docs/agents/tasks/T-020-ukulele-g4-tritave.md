---
id: T-020
title: No wrong note shown as certain when a high note's range hands down to Mid
status: done
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

`T-003e`'s recordings found a wrong note the octave guard marks as known. On `ukulele_g4` (392 Hz,
from High), the selector moves to Mid at 1.20–1.25 s. Mid then reads 131 Hz, a third of G4 (a
twelfth below, a C3), at clarity 0.85–0.90, with the octave marked **Known**: 20 frames that would
put a wrong note on screen as certain. The guard (`adr/0029`) checks octaves (`f/2`, `2f`); a
factor of 3 passes it. The owner made this its own task (2026-10-30).

## Context

Read: `docs/adr/0029-octave-guard.md`, `docs/AUDIO_ENGINE.md` §2–§4, `rust/crates/dsp/src/octave.rs`,
`rust/crates/dsp/src/chain.rs` (range selection), `rust/crates/dsp/tests/recorded.rs` (the
`ukulele_g4` row and its ceiling), `fixtures/audio/README.md`. Dsp work goes to the `dsp-engineer`
agent. Reproduce first: `cargo nextest run -p diapason_dsp --test recorded --no-capture`.

## Acceptance criteria

- [x] The cause is named, with the frames it happens in: why the selector leaves High while G4 is
  still sounding, and why Mid's reading passes the guard as `Known`. *(Implementation notes.)*
- [x] A synthetic test reproduces it before the fix (a high note decaying across the High/Mid
  boundary, strong third partial) and passes after. *(Steady G4 over a tone at G4/3; see
  Deviations.)*
- [x] `ukulele_g4` shows zero gross frames, and `recorded.rs` gates that, as it gates `bass_b0`.
  *(Now every ceiling fixture is gated on gross frames.)*
- [x] No other recorded fixture gains a gross frame, and no gold median moves past ±2 ¢. *(Every
  other row is identical.)*
- [x] Latency budgets in `AUDIO_ENGINE.md` §1 still hold (`tests/chain.rs`): every reported figure
  is identical.
- [ ] The bench gate holds. *Closed unproven: no quiet host this session (Verification); the owner
  reruns `just bench` on one. The change adds one comparison per verifier frame.*
- [x] Any change to the guard's rules supersedes the relevant part of `adr/0029`. *(`adr/0031`.)*
- [x] `just verify` passes, and all six CI jobs are green (the ruleset merges nothing otherwise).

## Out of scope

Budgets for the ukulele (`T-003g`). Smoothing, which would hide the symptom (`T-003f`).

## Implementation notes

**Cause** (traced on the fixture at 48 kHz, home High). From 1.1 s, two sounds sit under G4's first
partial (391 Hz, −2 dB): a tone at 134.8 Hz, −13 dB (the body, 54 ¢ above G4/3), and the C4 string
in sympathy at 261.5 Hz, −23 dB (G4's second partial is C4's third). G4's third partial is the
strongest peak by then. Mid, the verifier, reads their near-common period at 130.7 Hz, clarity
0.85–0.90. At 1.200 s two fresh agreeing Mid frames passed `observe_verifier`: below High's band,
and 391.25 = 3 × 130.69 (`harmonic`, k = 3). The chain switched; it was not a miss scan. Mid then
showed 130.7–131.6 Hz for 20 frames (1.2025–1.250 s) as **Known**: Mid can test its octave
(`tests_octave`), and `must_repeat` tests only the locked note's octaves (−1902 ¢ is class
+498 ¢). The watcher moved the chain back up at 1.2525 s. A second move at 1.305 s fell after the
reference window. High sees 130.7 Hz (down to 106.9 Hz) and read G4 throughout.

**Fix:** `observe_verifier` takes a candidate only if the current range cannot see it
(`RangeTable::sees`). The reasoning and the measurements are in `adr/0031`. Ten `ukulele_g4` frames
(1.250–1.2725 s) are now `ambiguous`: High's pick flips to G4/2 and the guard's τ/2 bridge shows the
held G4 (`adr/0029` (a)), so the letter and cents are right.

**Deviations.**
- The synthetic test is a steady G4 over a tone at G4/3 from silence, not a decaying note. Probed
  first, decaying G4s whose low tone ends louder than about −10 dB become a chord High itself
  reads at G4/3 (through `observe`, not the verifier). That is `T-014`'s case, recorded in its
  task file. The test holds the recording's levels: body −11 dB, C4 −20 dB.
- The plan said to commit the test red first. `main` is rebase-merged and keeps every commit, so
  the fix landed first. The red run is recorded below instead.
- `recorded.rs` gates gross frames on every ceiling fixture rather than on a list of two. All
  fourteen were already at zero, and a wrong note is no instrument's regression to allow.

## Verification performed

- **Before the fix:** `ukulele_g4` 20 gross, median −2.583 ¢. The new chain test failed 20 of 24
  cases (up to 577 wrong frames each), and the new `range` unit test failed.
- **After:** `ukulele_g4` 0 gross, 10 ambiguous, median −2.577 ¢. The other 23 recorded rows are
  identical. All 127 `diapason_dsp` tests pass (two new).
- **Diffed** every line `tests/chain.rs` reports, before and after: all gated figures are
  identical. Only ungated `T-014` chords moved (A2 ringing loud over a bass E, 1–8 frames either
  way). Then a throwaway probe of 108 cases: weak-fundamental and pinch notes at 51.9–73.4 Hz (home
  Mid and High) and 108–146.8 Hz (home High), from silence and after a note two octaves up. Output
  was identical before and after: no wrong frame, same latencies.
- `just verify`: every step green (196 workspace tests, clippy, drift, deny, fixtures check) except
  `docs-check`. That one fails only on the session-end items this task file and STATE fix.
- `just bench`, twice, on a busy host. Run 1: everything within 10 % except `chain/octave`, +12 %
  on 35 ns, whose code (`octave.rs`, `pitch.rs`) this branch does not touch (STATE's µs trap).
  `chain/analysis`, which runs the changed selector, was −1.4 %. Run 2, with Chrome at 94 % of a
  core, put untouched code (`band_limit`, frame assembly) 23–48 % over: noise, not a result.
