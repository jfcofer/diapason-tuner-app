---
id: T-003c
title: Octave guard across estimates and ranges, inside the latency budgets
status: done
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
- [x] **Downward note changes inside §1's fail lines**, measured from note onset with the
  previous note stopped, and with it ringing at least 12 dB below the new note at onset (the
  string being tuned is plucked; the louder chord case is only recorded):
  - 80–160 Hz ≤ 150 ms; *holds, stopped, after a gap and 12 dB under (≤ 118.3 ms);*
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
- [x] `just verify` passes, and all six CI jobs are green (#34, rebase-merged 2026-10-30).

## Out of scope

- Median-of-5, one-euro filter, the split between `frequency_raw` and `frequency_display`, lock
  detection, and §1's display-stability budget: all `T-003f`. `T-003c` must stay inside the
  ledger's chain line; the smoothing reserve belongs to `T-003f`.
- Recorded fixtures (`T-003e`).
- Engine, FFI and UI (M3).

## Implementation notes

**Baseline**, before any change: the table is in `journal/2026-10-30-t-003c-octave-guard.md`;
its headline numbers are in `adr/0029`'s Context.

**Design** (`adr/0029`, `AUDIO_ENGINE.md` §1–§3), by `dsp-engineer`: `dsp::octave::OctaveGuard`,
`Mpm::peak_near`/`value_at`, verifiers (Mid and Low under High) replacing the Low-only reference,
`RangeTable::tests_octave`, `chain::Frame::octave`, fixtures in `synth::partials`. Rules beyond the
plan, each measured: the lowest note, clarity bars, sticky doubt, the repeat test.
- **Round 3** (2026-10-24): the ringing-string rule binds the selected range too; the guard tracks
  the string left below and the octave above entering (`RISE`); verifier readings pass the tests.
- **Round 4** (2026-10-25, d009555): owner, **single notes win over chord rejection**. The 0.85 bar
  binds only blind estimates (`AFTER_NOTE_CLARITY`); the locked note is forgotten after Low's
  window unheard (`OctaveGuard::heard`); guard-rejected frames are `Withheld`, not misses; a
  watcher range moves the chain up to a clearer note entering over a held one.
- **Round 5** (2026-10-25): faint in-band frames are `Withheld`; `Observation::Faint` and its
  `fading` state removed. Since round 6, no gated test needs the faint rule (mutation to a miss or
  to a pitch: all pass); kept, as it costs nothing, and noted for `T-003f`.
- **Round 6** (2026-10-30): the reviewer's ~50 scratch probes were run at both rates (outcome
  matrix in the journal), then deleted. They found one gated miss: **a gap shorter than the gate's
  250 ms hold** scanned the chain away from the next note's range (A3 → A2 after 0.15 s: 150.5–
  155.4 ms, fails at 150; low B up to 271 ms). Fix: `gate::GateState` crosses the ring instead of
  a `bool`, and frames with the gate open only by its hold are `Observation::Holding`, no miss.
  Gated: `a_downward_change_after_a_gap_the_hold_spans_meets_the_fail_line`.

**Deviation 1 — gate moved to `T-003f`** (owner, 2026-10-23): A2 → bass E, 12 dB under, τ 3 s
(393.3 / 466.3 ms) and A2 → low B, 12 dB under, τ 1.5 s (648.3 / 421.4 ms) miss 260 ms on cents;
ceilings: those + 20 ms. **Deviation 2 — only A2 over a bass E is a gated chord** (owner,
2026-10-25): A2 over a low B, detuned chords, consonant intervals and other loud pairs are recorded
(`Gate::Recorded`), `T-014-consonant-chords`. **Deviation 3:** onsets after a short gap wait for
their own scan: A2 → bass E after 0.1 s 85 / 88.4 ms (was 50 / 53.5); E4 ringing loud over low E,
recorded only, needle 115 ms (was 12.5).

**Recorded, not gated (round 6 probes):** a 200 ¢ bend in 0.1 s lags 19–20 frames past 50 ¢ (the
window, not an octave); a note at −50 dBFS from home Mid reaches Low ~20 ms later than before the
hold fix (low B 123 ms, inside 260), and while the hold runs a quiet note cannot be scanned to
(two ranges up, only by the scan); the octave above a string 6–8 dB under (a chord) shows the
lower note; dense no-gap fifths down into a weak fundamental (73.96 → 49.31 Hz) show 1–3 wrong
frames, outside the gated sweep's notes: `T-003f`'s median of five is the next line of defence.

## Verification performed

2026-10-30, Ryzen 7 8840HS, rustc 1.98.0, host only.
- **Gates** (both rates): leap sweep 0 wrong; downward changes per §1, including gaps under the
  hold (A3 → A2 78–101 ms, A1 → low B 129–180 ms in all); octave above a ringing string (10–12 dB)
  0 wrong; the gated chord no third note.
- **Held-out probes** (round 6, release, both rates): after-gap sweep over 10 note pairs, 4 signals,
  4 gaps and 48 phase/offset pairs: 0 fail-line misses (10 rows before the fix). Octave above a
  ringing string, 12 notes × 3 signals at 10–12 dB: 0 wrong.
- **Mutations**, each failing a test: rounds 1–3 as before; round 6: holding frames as misses.
- **Benches** (`just bench`, quiet host): no entry over 10 %; analysis 74.7 µs per 10 ms in High.
  `just verify` passed: 171 tests and the doctests. (A run beside the reviewer read +5–10 % everywhere.)
