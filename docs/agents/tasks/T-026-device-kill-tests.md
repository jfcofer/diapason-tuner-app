---
id: T-026
title: Measure the detector and hear the microphone on the Redmi, before any wiring
status: done
milestone: M2
owner: claude
created: 2026-11-01
---

## Goal

Nothing in `AUDIO_ENGINE.md` §1 has been measured on a phone, and every recording so far came from a
library, not a phone microphone. This is the cheapest test that can overturn the design: it needs no
threads and no UI. A failure here moves `adr/0028`'s hops, or the chord tests of `adr/0033` and
`adr/0034`, before anything is built on them.

## Context

Read: `docs/AUDIO_ENGINE.md` §1, `docs/adr/0030` (recordings), `docs/adr/0033`, `docs/adr/0034`,
`tools/test-android-device.sh`. Needs the Redmi attached and the owner playing a guitar. HyperOS wants
a tap on every USB install (`DEVELOPMENT.md` §6).

- **First step, unverified:** whether the criterion benches can run on the phone through
  `cargo-ndk-runner`. Reuse them: `dsp`'s lint list bans `Instant::now`, tests included.
- `chain/analysis` costs about 78 µs per 10 ms block on the host (`benches/baseline.json`), so 0.78 % of
  a core there. The phone's figure is what this task finds.
- The Redmi grants the VoiceRecognition preset and refuses Unprocessed (`T-002b`). `STATE.md`'s open
  questions already suspect its high-pass may hide a real low string under its harmonic.

## Acceptance criteria

- [x] `just verify` passes; all six CI jobs green.
- [x] A `just` recipe runs the `dsp` benches on an attached device, or the task records why it cannot
      and what measures the same cost instead.
- [x] Recorded on the Redmi, in release: frame cost per range (budget ≤ 1.5 ms, fail 4 ms) and the
      duty cycle of `Analysis` at its real hop (budget ≤ 8 % CPU, fail 15 %). The owed 4096-point frame
      from `T-003a` is included.
- [x] A dev-only capture path records the microphone through `AAudioBackend` to a WAV. It is not in
      the prod flavour.
- [x] The owner's captures: each open string, low E repeated, a fifth, a fourth, and strums. Replayed
      through `recorded.rs`, with per-frame results recorded. Any checked in follow `adr/0030`.
- [x] The gate's outcome is written here, and STATE's order changed if it says so:
  - [x] frame cost fails → revisit `adr/0028` by a new ADR before `T-027`: **passes**, but CPU fails (`T-036`);
  - [x] a real single string withheld by `adr/0033`/`adr/0034` → fix that next: **misread, not withheld** (`T-035`);
  - [x] `T-023` and `T-003g` are re-scoped or abandoned from this evidence: **both re-scoped**.

## Out of scope

Wiring the detector into the engine (`T-027`). Tuning thresholds in this task.

## Implementation notes

**The outcome.** `T-035` (a real low E reads E3) comes first, then `T-036` (CPU on the A55), then `T-027`.

**Benches, 2026-11-01** (`just bench-android --runs 3 --save`, `benches/baseline-redmi.json`). Helio G99,
6 × A55 at 2.0 GHz, 2 × A76 at 2.2 GHz. Thermal status 0 throughout, skin 39.4–41.5 °C, screen awake.

| Median of three | A55 | A76 | All cores | Target / fails at | Verdict (A55) |
|---|---|---|---|---|---|
| `chain/frame/low` | 0.23 ms | 0.07 ms | 0.07 ms | 1.5 / 4 ms | ok |
| `chain/frame/mid` | 0.05 ms | 0.02 ms | 0.02 ms | 1.5 / 4 ms | ok |
| `chain/frame/high` | 0.11 ms | 0.03 ms | 0.03 ms | 1.5 / 4 ms | ok |
| `nsdf/4096` (owed by `T-003a`) | 0.60 ms | 0.19 ms | 0.18 ms | 1.5 / 4 ms | ok |
| `chain/analysis`, CPU | 13.4 % | 4.4 % | 4.3 % | 8 / 15 % | over target |
| `chain/strum`, CPU | 13.5 % | 4.4 % | 4.5 % | 8 / 15 % | over target |
| `chain/chord`, CPU | **15.9 %** | 5.8 % | 5.7 % | 8 / 15 % | **fails** |

**The gate, fixed before any number existed, fails on the A55.** Frame cost passes with room. Duty cycle does
not: a single note misses the target and two strings ringing cross the fail line. The A55 is the verdict
because nothing pins the analysis thread. These omit `Front` (`T-027`) and everything above `dsp`, and a saturated
bench presumably holds the core near its top frequency (the shell cannot read the cap), so only `T-031` can
measure the thread's real load under the governor.

**The chord's fail is real by the gate, and marginal.** A later single pass, testing the script on a cooler phone
(skin 34–37 °C, screen partly off), read 12.5 % and 14.3 % where this median read 13.4 % and 15.9 %. Thermal status
was 0 both times. The gate stands on the three-pass median, and `T-036` must control the conditions.

**Where the A55's 1338 µs per 10 ms goes,** from the frame costs and each range's hop (arithmetic, not a
profile; it sums to 1249 µs): Low verifier 2 × 235, High 4 × 106, Mid verifier 4 × 48, the two filters 163.

**Captures, 2026-11-02** (acoustic or electric not recorded; 17 takes at 48 kHz, no audio loss reported, none
clipped). Kept outside the repo (`just capture-session`), replayed from home Mid with `just replay`. Counts are
frames the needle would show. "Flagged" is `ambiguous octave`, which the spec allows.

| Take | Frames | Right | Octave off, flagged | **Octave off, known** | Other note | Blank | First − second partial |
|---|---|---|---|---|---|---|---|
| `e2` | 2288 | 258 | 406 | **410** | 41 | 1173 | −15.4 dB |
| `e2-2` | 2362 | 1267 | 281 | **251** | 4 | 559 | −17.0 dB |
| `e2_repeat` | 4382 | 2493 | 675 | **380** | 68 | 766 | −16.7 dB |
| `e2_repeat-2` | 4195 | 2664 | 442 | **326** | 54 | 709 | −16.2 dB |
| `e2_soft` | 3644 | 2509 | 465 | **367** | 50 | 253 | −12.8 dB |
| `e2_hand` | 3205 | 2937 | 138 | **2** | 18 | 110 | −13.8 dB |
| `a2` | 3677 | 3495 | 16 | 0 | 1 | 165 | −11.2 dB |
| `d3` | 2594 | 2529 | 0 | 0 | 0 | 65 | +0.5 dB |
| `g3` | 1195 | 951 | 0 | **6** (G4) | 0 | 238 | −8.5 dB |
| `b3` | 1093 | 809 | 0 | 0 | 0 | 284 | +14.5 dB |
| `e4` | 1400 | 852 | 26 | 0 | 21 | 501 | +10.2 dB |

- **Low E is a live bug.** Five takes of six show E3 with its octave known on 8–18 % of frames. Nearly all come
  after the selector moved to High; in Mid the same string reads E2. `g3`'s six G4 frames, also in High, may be
  the same bug. `T-035` holds these.
- **Low E's fundamental is weak here, and the cause is open.** Library, same measure: E2 steel −9.4, nylon −3.3,
  electric +0.1; A2 steel +3.3, electric +0.5; G3 steel +8.6 dB. A2, at 110 Hz inside Android's 100 Hz–4 kHz band,
  is further from the library than low E is from the nearest, which argues against a simple high-pass. Guitar,
  room and distance all differ too. `e2_soft` has the strongest fundamental of the six and still fails, while
  `e2_hand` barely does: this ratio alone does not explain the bug. The room's noise falls below 100–150 Hz.
- **The gate holds.** No frame in 30 s of silence. On notes it stays open to within a few dB of the floor (RMS
  about −60 dBFS). Treble strings reach it in about 2.5–3.4 s at 30 cm, which is the sustain a tuner has here.
- **`Frame::held` on single strings** falls at the ends of takes, not in a note's body, worst on B3's tail (149
  frames). A cost to sustain, not a wrong note: no separate fix yet.
- **Chords.** The fourth shows A1, which neither string plays, on 651 of 742 needle frames. The fifth shows A2 on
  1751 frames and A1 on 140. The open strum shows A2 or E3, and a slow strum mostly A2. `T-023` is real.
- **Pitch medians** run E2 −5 to −8 ¢ and B3 +5.6 ¢. There is no truth for them until a cut has a reference
  (`adr/0030`), so they say nothing yet about accuracy. No take is checked in: `T-035` needs the first ones.

**Two limits on "no audio lost".**
- Input xruns read 0 on all 17 takes, but `AAudio.h` lets an input device always return 0, and the Redmi's path
  may do so. An overrun keeps a take's length, so a lost stretch is not ruled out. Short blocks and ring overflow
  are real counts, both 0.
- Silent-take rejection is unproven. `silenced` was kept at −43.6 dBFS, and whether microphone access was really
  off is an open question to the owner. Rejection now looks for a 100 ms run of exact zeros, not a peak of zero.

**A correction from review.** The first partial figures here were a measurement artefact: a Goertzel at exactly
2f₀ over the whole span misses a partial a few cents sharp. They flipped the sign on `e4`, and were repeated to the
owner as "11–23 dB, against 5 dB". `replay` now finds each partial's peak in 0.5 s frames. Equal partials 0, 2 and
8 ¢ apart read 0.0 dB, and an independent decoder agrees with these figures.

**Deviations.**
- Benches are built with plain `cargo bench` and the NDK's clang, not `cargo ndk`: it swallows cargo's JSON,
  which names the binaries, and `cargo ndk-env`'s environment does not link outside it.
- The script prompts for each take, not the agent: the owner keeps both hands on the guitar.
- Raw takes go through `just replay`, on `recorded.rs`'s own glue (`tests/common::Tuner`). Only checked-in
  cuts go through `recorded.rs`, since the rest stay outside the public repo.

**Corrections to the approved plan.** Its "53.5 °C" CPU reading was stale: `dumpsys thermalservice`'s cached
values never moved, and the HAL's current CPU reading is `-274` (no sensor). Skin is logged instead. Its
"8–12 %" guess for the A55 was low. The planned silent-take test, by toggling the phone's microphone privacy
from `adb`, was denied at its permission prompt. The owner's own attempt was kept rather than rejected: see above.

## Verification performed

- `replay` on `fixtures/audio/steel_e2.wav`: 82.472 Hz against the manifest's 82.391, +1.7 ¢, where
  `recorded.rs` reports +1.82 ¢ over its narrower window.
- A 3 s room take on the Redmi: 48 kHz, burst 960, `VoiceRecognition` obtained, nothing lost, peak −16.4 dBFS.
- 2026-11-02: the 17 takes through `just replay`; `just verify` and CI on this PR.
