---
id: T-003f
title: Smooth the estimate for display, detect lock, inside the ledger's smoothing reserve
status: done
milestone: M2
owner: claude
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
  its two rows "Gated in `T-003f`") and §3 (smoothing, lock detection, the octave guard);
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

- [x] `frequency_raw` (unsmoothed, for lock and the strobe) and `frequency_display` (median of
  five, then a one-euro filter, `min_cutoff = 0.6`, `beta = 0.02`) per §3, allocation-free and
  panic-free (`tests/no_alloc.rs`).
- [x] **Inside the reserve:** onset latency on `frequency_display`, measured as the ledger test
  measures it, is within each range's chain line plus its smoothing line, at 48 and 44.1 kHz.
- [x] **Display stability** (§1): a held synthetic note wobbles ≤ 1 ¢ peak to peak on
  `frequency_display`, at both rates; the test names the fixture.
- [x] **Lock detection** (§3): |cents| ≤ the tolerance (default ±3, 1–10) for ≥ 250 ms of
  consecutive frames with clarity > 0.8, unlocking at tolerance + 1.5 ¢, on the audio clock only.
  Tests cover lock, no lock, and no chatter at the boundary.
- [x] **The guard survives smoothing:** on `frequency_display`, the no-gap leap sweep and the
  downward changes show zero judged wrong frames, and chords no third note (`T-003c`'s rules,
  unchanged). The `ambiguous octave` flag reaches the displayed value.
- [x] **The two cases `T-003c` moved here meet §1's 260 ms fail line on the smoothed needle,** at
  both rates: A2 → bass E with the A2 ringing 12 dB under, τ 3 s; and A2 → low B with the A2
  ringing 12 dB under, τ 1.5 s. The `T-003c` ceilings (`Gate::Shortfall` in `tests/chain.rs`)
  become that gate, and the figure against the 190 ms target is recorded in §1.
- [x] A criterion bench for the smoothing per frame; the CPU figure is re-recorded.
- [ ] `just bench` holds on a quiet host, `T-020`'s selector change included (`T-020` could not
  get one). **Closed, as an owner action:** the only host is the owner's working laptop, and
  three runs here could not get it quiet (see Verification). An agent cannot quiet it, and must
  not re-baseline on noise, so it stands in STATE's next steps as the owner's `just bench`, then
  `just bench-baseline` to adopt `chain/smooth`.
- [x] `just verify` passes, and all six CI jobs are green (#41; `pr-check` holds them).

## Out of scope

- Engine, FFI and UI wiring (M3).
- Recorded fixtures (`T-003e`) and the note table (`T-003d`).
- Changing the octave guard's rules (`adr/0029`): a change there needs its own ADR.

## Implementation notes

**Built:** `dsp::smooth` (`Median`, `OneEuro`, `Needle`) and `dsp::lock::Lock`.
`chain::Frame::display` is the needle; `Frame::estimate` stays `frequency_raw`. Every rule and
measurement is in `adr/0032`.

**Deviations.**
- **The needle never trails the median by more than 3 ¢** (`smooth::MAX_LAG_CENTS`), a rule §3 did
  not have. Measured first, as the plan required: the spec's filter alone settled up to 18.8 ms past
  the ledger's smoothing line (Low, a sine low B, 44.1 kHz). An onset converges from as far as 45 ¢,
  and the median already spends the two hops on that ramp. Faster parameters met the line, but
  wobbled 6.3 ¢ at 10 dB SNR. Re-seeding met both only between 4 and 5 ¢, and jumped the needle.
  The bound meets both from 3 to 4 ¢. `min_cutoff` and `beta` are §3's, in cents.
- **`d_cutoff = 1` Hz**, the paper's default, which §3 does not state.
- **Lock takes cents from its caller.** The target needs the note table (`T-003d`) and the
  session's settings (§4). `dsp` does not guess them.
- **The latency gates moved to the needle.** `onset_latency_is_within_the_ledger` gates both the
  estimate (chain line) and the needle (chain + smoothing). The boundary, downward-change and
  held-gap tests total the needle plus the lines after the smoothing. Wrong-frame gates count both
  values. The stopped and gap totals equal `T-003c`'s; the 12 dB-under ones fell, because the
  median removes the frames that wandered.
- **Review round 1** (the `reviewer` subagent, with a scratch probe of all 24 recordings):
  - **An octave change alone reset the needle.** The guard settling a held note's octave blanked
    it 2–27 times a recording, in six of them, all in decays after the judged windows, which
    `recorded.rs` did not read. Now the held note in another octave moves the history by 2ᵏ. A
    stray frame of another note gathers apart instead of resetting (`nylon_e2`'s attack), and the
    history outlives five frames without the note (`HELD_GAP_FRAMES`), where it had outlived any
    gap the gate held. `recorded.rs` now gates the needle: no gross needle frame in the window,
    and no flicker over the whole recording. With the octave rule removed, that gate fails on six
    fixtures.
  - **"Two hops" held only worst case against worst case.** Run by run the needle lagged its
    estimate by up to 30 ms (Low). The ledger's lines are worst cases, and that is what is gated.
    The docs that claimed more are fixed, and the ADR says so.
  - Also fixed: the claim that every total equals `T-003c`'s; the lock and needle disagreement left
    to M3 (ADR); `smooth::Display` renamed `Needle`, which no longer shadows `fmt::Display`; the
    held-note test's doc; doc lines over 100 columns.
- Done inline, not through the `dsp-engineer` subagent the plan named. The work needed the
  measurements in this session's context.

## Verification performed

- **Decision gate (plan step 1):** on the needle, before any test change, A2 → bass E (12 dB under,
  τ 3 s) totalled 163.3 / 166.9 ms, and A2 → low B (τ 1.5 s) 163.3 / 191.9 ms, against 260 ms.
  Zero judged wrong frames.
- **Parameter sweep** (scratch tests, removed): worst needle excess over the line 18.8 ms with the
  spec's filter; −0.2 ms with the bound at 1–4 ¢; +3.0 ms at 4.5 ¢. Worst 10 dB-SNR wobble: 0.70 ¢
  with no bound, 0.92 ¢ at 3 ¢, 2.83 ¢ at 2 ¢.
- **Gates:** `onset_latency_is_within_the_ledger` (needle worst 123.75 ms in Low against 132.69, and
  26.71 ms in High against 26.92); `a_held_note_holds_the_needle_within_a_cent` (≤ 0.12 ¢ at
  20 dB SNR where the estimate wobbles 2.22 ¢; 0.92 ¢ at 10 dB, recorded); lock unit tests (lock at
  exactly 250 ms, no lock, no chatter, the audio clock across hops, the tolerance clamp);
  `a_note_in_tune_locks_on_the_audio_clock_and_one_out_of_tune_does_not` (+1 ¢ locks 262–344 ms
  after the onset, +4 ¢ never); `the_needle_and_the_lock_never_allocate`; `recorded.rs`, 24
  recordings, no gross needle frame and no flicker (before the octave rule: `ukulele_e4` 27,
  `ukulele_g4` 16, `ukulele_a4` 4, `nylon_e4` 3, `steel_e4` 2, `steel_g3` 2). Every figure above
  re-measured after review round 1, unchanged.
- `just verify`: green.
- **`just bench`, three runs on a busy host** (load average 40 after `verify`, then a JVM at 15–49 %
  of a core and Chrome). Untouched benches moved +68 % (`nsdf/8192`), then +88 % (`band_limit`).
  `chain/analysis`, which runs the needle and `T-020`'s selector, read +14.6 % in run 1 (load 40)
  and −0.7 % in run 2: 77.2 µs per 10 ms, 0.77 % of a host core (was 0.78 %). `chain/smooth`: 52 ns
  a frame. Not a quiet-host result, so no baseline was saved.
