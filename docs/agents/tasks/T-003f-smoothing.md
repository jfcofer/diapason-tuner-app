---
id: T-003f
title: Smooth the estimate for display, detect lock, inside the ledger's smoothing reserve
status: todo
milestone: M2
owner: unassigned
created: 2026-10-23
---

## Goal

`T-003c` delivers one estimate per frame, octave-guarded, and `T-003b`'s ledger reserves two hops
for what comes next. This slice turns that stream into what the needle shows. The two values of
`AUDIO_ENGINE.md` §3 (`frequency_raw`, `frequency_display`), median of five then a one-euro
filter, and lock detection. It meets §1's display-stability budget without spending more latency
than the ledger reserves. It also owns the 260 ms needle gate of two downward changes that
`T-003c` misses on cents, not octave, because a median of five removes the single frames that
miss.

## Context

Read first:
- `docs/AUDIO_ENGINE.md` §1 (budgets, the ledger's smoothing reserve, the downward-change table and
  its two cases "not met") and §3 (smoothing, lock detection, the octave guard);
- `docs/adr/0028` (ledger, `SMOOTHING_HOPS`) and `docs/adr/0029` (octave guard, `ambiguous octave`).

Prior art:
- `dsp::chain::Frame` (`estimate`, `octave`);
- `dsp::range::SMOOTHING_HOPS` and `Ledger::smoothing`;
- `tests/chain.rs`: `onset_latency_is_within_the_ledger`,
  `a_downward_change_meets_the_fail_lines_and_never_shows_a_wrong_octave` and its
  `Gate::Shortfall` cases;
- `T-003c`'s task file, Deviation 1.

The owner's decision of 2026-10-23: an `ambiguous octave` frame moves the needle, and shows the
letter and the cents without the octave number.

Known trap: a median over frames that straddle an octave change or a `None` mixes estimates the
guard kept apart. Smoothing must not undo the guard.

## Acceptance criteria

- [ ] `frequency_raw` (unsmoothed, for lock and the strobe) and `frequency_display` (median of
  five, then a one-euro filter, `min_cutoff = 0.6`, `beta = 0.02`) per §3, allocation-free and
  panic-free (`tests/no_alloc.rs`).
- [ ] **Inside the reserve:** onset latency on `frequency_display`, measured as the ledger test
  measures it, is within each range's chain line plus its smoothing line, at 48 and 44.1 kHz.
- [ ] **Display stability** (§1): a held synthetic note wobbles ≤ 1 ¢ peak to peak on
  `frequency_display`, at both rates; the test names the fixture.
- [ ] **Lock detection** (§3): |cents| ≤ the tolerance (default ±3, 1–10) for ≥ 250 ms of
  consecutive frames with clarity > 0.8, unlocking at tolerance + 1.5 ¢, on the audio clock only.
  Tests cover lock, no lock, and no chatter at the boundary.
- [ ] **The guard survives smoothing:** on `frequency_display`, the no-gap leap sweep and the
  downward changes show zero judged wrong frames, and chords no third note (`T-003c`'s rules,
  unchanged). The `ambiguous octave` flag reaches the displayed value.
- [ ] **The two cases `T-003c` moved here meet §1's 260 ms fail line on the smoothed needle,** at
  both rates: A2 → bass E with the A2 ringing 12 dB under, τ 3 s; and A2 → low B with the A2
  ringing 12 dB under, τ 1.5 s. The `T-003c` ceilings (`Gate::Shortfall` in `tests/chain.rs`)
  become that gate, and the figure against the 190 ms target is recorded in §1.
- [ ] A criterion bench for the smoothing per frame; the CPU figure is re-recorded.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

- Engine, FFI and UI wiring (M3).
- Recorded fixtures (`T-003e`) and the note table (`T-003d`).
- Changing the octave guard's rules (`adr/0029`): a change there needs its own ADR.

## Implementation notes

## Verification performed
