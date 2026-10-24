---
id: T-003c
title: Octave guard across estimates and ranges, inside the latency budgets
status: in-progress
milestone: M2
owner: claude
created: 2026-10-23
---

## Goal

`PRODUCT_SPEC.md` §2 promises that the display "never flickers between octaves", and octave errors
are the failure players notice first. `T-003b` showed that this cannot be fixed in the range
selector alone. Each heuristic it tried there traded one octave error for another:
- a weak fundamental read an octave up in High;
- a gapless note change read a common sub-period in Low;
- a ringing previous string held the wrong octave for up to 655 ms.

On 2026-10-23 the owner moved octave correctness across ranges here. This task designs the
estimate-level guard (`AUDIO_ENGINE.md` §3) and the range guard together, and gates them against
§1's budgets, including downward note changes, which `T-003b` measured but did not gate.

## Context

Read first:
- `docs/AUDIO_ENGINE.md` §1 (budgets, the ledger, which measures from silence), §2 (ranges, the
  selector, the Low reference) and §3 (octave guard);
- `docs/adr/0028`;
- `T-003b`'s deviations and its **measured octave-trap baseline** (the starting point to beat);
- `PRODUCT_SPEC.md` §2: the `ambiguous octave` state.

Prior art:
- `dsp::range::RangeSelector`, with its freshness, agreement, `MAX_HARMONIC` and stuck-state bound;
- `dsp::chain::Analysis`;
- `synth::weak_fundamental` and the ringing-first-note fixtures;
- the no-gap leap sweep and the octave tests in `tests/chain.rs`;
- the reviewer's probes in `T-003b`'s notes.

**Owner decision, 2026-10-23: an `ambiguous octave` frame moves the needle.** An octave error
keeps the pitch class and the cents, so a frame flagged ambiguous shows the letter and the cents,
without the octave number, and it counts for §1's latency budgets. The time until the octave is
known is recorded and bounded separately. The plan: an estimate whose range cannot test 2τ stays
ambiguous until the range below, which can, confirms or moves it (`adr/0029`, this task).

Known trade-off: a Low window spans about 100 ms, so any evidence it gives about a note change
arrives late. The design must not depend on one window alone. Candidates to evaluate are the
τ/2 and 2τ test on the current frame, the NSDF's sub-harmonic peaks, the previous accepted
estimate, and multi-frame confirmation.

## Acceptance criteria

- [x] **Estimate-level guard** (§3 (a)): before a peak is accepted, a peak of comparable clarity
  at τ/2 or 2τ is tested, and the one consistent with the previous accepted estimate is
  preferred. "Comparable" is a stated, tested threshold.
- [x] **Jump hysteresis** (§3 (b)): a jump of more than 600 ¢ from a locked estimate needs three
  consecutive confirming frames. A test covers both the case where it holds and the case where
  it gives way.
- [x] **The range guard is reworked with it**, so that one design decides octave and range.
  `RangeSelector` keeps the stuck-state bound from `T-003b`.
- [x] **No octave flicker:** zero clear frames more than 50 ¢ off once judged (`T-003b`'s judging
  rule, unchanged). A frame may instead be `None` or flagged `ambiguous octave`. This holds at 48
  and 44.1 kHz, across:
  - the no-gap leap sweep (up and down by a fourth, a fifth and an octave), with sine, sawtooth
    and weak-fundamental signals;
  - the ringing first note, with τ of 0.6, 1.5 and 3 s. While two non-harmonic notes sound,
    Low reads their common period (A2 over bass E gives 27.5 Hz, `T-003b`): that is a chord, so
    the answer is `None` or ambiguity, never a third note;
  - a pinch harmonic (strong 2nd and 3rd partials) and low B (30.87 Hz, weak fundamental).
  *Holds.* Chords (A2 over a bass E or a low B) show `None`, ambiguity, or the old note still
  ringing (two frames, τ 3 s), and no third note, known or ambiguous (Deviation 2).
- [ ] **Downward note changes inside §1's fail lines**, measured from note onset with the
  previous note stopped, and with it ringing at least 12 dB below the new note at onset (the
  string being tuned is plucked; the louder chord case is only recorded):
  - 80–160 Hz ≤ 150 ms; *holds, stopped, after a gap and 12 dB under (≤ 120.8 ms);*
  - low B ≤ 260 ms. *Holds, except two 12 dB-under ringing cases (Deviation 1).*
  
  The figure against the targets (110 and 190 ms) is recorded. §1 and ADR 0028 state it, and the
  "measured from silence" caveat that `T-003b` added is closed.
  *Closed: the two 12 dB-under ringing cases' 260 ms needle gate moved to `T-003f`, owner
  2026-10-23; here they are held to regression ceilings (`Gate::Shortfall`).*
- [x] **`ambiguous octave`:** when the guard cannot decide within its window, the estimate says so
  (a flag, not a guess), with a test. One case is ambiguous by nature: a note an octave above a
  still-ringing lower string. `T-003b`'s patience rule moves that case down after 250 ms; this
  task decides it on purpose.
- [x] Every new per-frame path is allocation-free and panic-free (`tests/no_alloc.rs`), and the
  CPU figure is re-recorded.
- [x] Any decision that closes off an alternative gets an ADR, or a superseding one if it
  contradicts 0028.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

- Median-of-5, one-euro filter, the split between `frequency_raw` and `frequency_display`, lock
  detection, and §1's display-stability budget: all `T-003f`. `T-003c` must stay inside the
  ledger's chain line; the smoothing reserve belongs to `T-003f`.
- Recorded fixtures (`T-003e`).
- Engine, FFI and UI (M3).

## Implementation notes

**Baseline**, before any change (`tests/chain.rs` at the baseline commit; 48 / 44.1 kHz, home
Mid, new note weak, partials 0.3/1/0.4). Wrong: judged clear frames off the new note, under the
leap rule. Needle: right pitch class within ±5 ¢ and stays; total adds the target's other ledger
lines (Mid 58.3, Low 63.3 ms). Before `T-003c` every frame's octave counts as known.

| Case | Wrong | Needle (ms) | Total (ms) | Octave known (ms) |
|---|---|---|---|---|
| E4 → low E, stopped | 19 / 21 | 140 / 134.9 | 198.3 / 193.3 | 140 / 134.9 |
| E4 → low E, 0.1 s gap | 0 / 0 | 27.5 / 27.3 | 85.8 / 85.7 | same |
| E4 → low E, rings (τ 0.6–3 s) | 41 / 40 | 7.5 / 1.4 | 65.8 / 59.8 | 120 / 119.7 |
| E4 → low E, 12 dB under (τ 0.6, 1.5, 3) | 44 / 46 | 125, 107.5, 107.5 / 113.6 | 183.3, 165.8 / 172.0 | 130 / 134.9 |
| A2 → bass E, stopped | 24 / 26 | 160 / 158.5 | 223.3 / 221.8 | same |
| A2 → bass E, 0.1 s gap | 0 / 0 | 50 / 53.5 | 113.3 / 116.8 | same |
| A2 → bass E, 12 dB under (τ 0.6, 1.5, 3) | 18, 19, 18 / 25, 22, 22 | 165, 185, 330 / 203.4, 163.5, 402.9 | 228.3, 248.3, 393.3 / 266.7, 226.8, 466.3 | same |
| A2 → bass E, rings (chord, τ 0.6, 1.5, 3) | 72, 199, 282 / 70, 197, 283 | 1015 / 1051; never | — | same |
| A2 → low B, stopped | 10 / 12 | 100 / 94.3 | 163.3 / 157.6 | 100 / 98.6 |

Chords show a third note in 65–270 frames (A0). Weak octave leaps down: 18–26 wrong; others 0.

**Design** (`adr/0029`, `AUDIO_ENGINE.md` §1–§3), by `dsp-engineer`: `dsp::octave::OctaveGuard`,
`Mpm::peak_near`/`value_at`, verifiers (Mid and Low under High) replacing the Low-only reference,
`RangeTable::tests_octave`, `chain::Frame::octave`, fixtures in `synth::partials`. Each rule
beyond the plan measured: the lowest note, clarity bars, sticky doubt, the repeat test.
**Review round 3** (2026-10-24): the ringing-string rule binds the selected range too, the guard
tracks the string left below and the octave above entering (`RISE`), and after a note a new note
needs 0.85 and verifier readings pass the same tests. Margins set on the review's fixtures,
checked on held-out ones (other notes, ±7/22/45 ¢): every gated case passes.

**Deviation 1 — gate moved to `T-003f`** (owner, 2026-10-23): A2 → bass E, 12 dB under, τ 3 s
(393.3 / 466.3 ms) and A2 → low B, 12 dB under, τ 1.5 s (648.3 / 421.4 ms) miss 260 ms on cents;
ceilings: those + 20 ms. **Deviation 2 — chords are gated for the families tested only**: A2 over a
bass E or low B and E4 over low E, ±30 ¢. Consonant intervals (A2 + D3 → D1, D3 + G3 → G1, B3 + E4
→ E2) and other loud pairs (G2/low B, C3/bass E, A2 + 60–100 ¢ over bass E) still show third
notes: `T-014-consonant-chords`. **Deviation 3:** the rules after a note cost onsets after a gap
(A2 → bass E 85 / 83.4 ms, was 50 / 53.5) and E4 ringing loud (needle 60 ms, was 12.5).

## Verification performed

2026-10-24, Ryzen 7 8840HS, rustc 1.98.0, host only. `just verify` passed (169 tests, 31 doctests).
- **Gates** (both rates): leap sweep 0 wrong; downward changes per §1; detuned (8 offset pairs, 3
  families) and octave above a ringing string (6 pairs, 10–12 dB) 0 wrong, chords 0 third notes.
- **Held-out probes** (other notes, ±7/22/45 ¢, 4 octave pairs): every gated case passes; their
  loud chords are `T-014`. Ambiguity: G3 known 65.0 / 63.7 ms, low E 140–145. From silence: low B
  needle 75 / 68.7 ms from home Low, unchanged; boundaries ≤ 80.1 ms.
- **Benches** (baseline: median of five runs): analysis 77.7 µs per 10 ms in High, so 0.78 % of a
  core; octave 35 ns; frames low 12.0, mid 2.79 (bimodal 2.7 / 3.3), high 5.90 µs.
- **Mutations**, each failing a test: rounds 1–2 as before; round 3: no ringing check in
  `observe` or the verifier, no rise, no string-below, new-note bar 0, unfiltered verifiers.
  Two that failed nothing (delayed rise floor, presence threshold) were removed.
