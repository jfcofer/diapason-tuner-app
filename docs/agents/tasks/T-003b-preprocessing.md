---
id: T-003b
title: Pre-processing chain, measured windows and hops, and the latency ledger
status: in-progress
milestone: M2
owner: claude
created: 2026-10-22
---

## Goal

`T-003a` left two things this slice must settle before any tracking (`T-003c`) builds on the
detector. First, a harmonic-rich tone (the sawtooth) meets ±0.5 ¢ only once a filter keeps its
content well below the **analysis** Nyquist. Second, `AUDIO_ENGINE.md` §2's numbers disagree
with §1 and with themselves. This slice builds the pre-processing that `T-003a` lacks: DC block,
adaptive gate, band-limit and ×4 decimation, and frames per range. It then derives the windows,
the hops and the decimation crossover from measurement and from a stated latency ledger, not
from §2's table.

## Context

Read: `docs/AUDIO_ENGINE.md` §1–§3, §7–§8; `docs/adr/0018`, `0021`, `0027`; `T-003a`'s two
deviations; `docs/TESTING.md` §2. `dsp` gains no dependency (`tools/check-deps.sh`).

What §2 gets wrong, found while planning (2026-10-22):

1. **Windows.** §2 asks for three periods of the range's lowest note, then tabulates
   2048/4096/8192 at 12 kHz: 8–27 periods, 171–683 ms. `T-003a` used the rule (Deviation 1);
   the table was never fixed.
2. **Hops versus §1's latency.** A 512 hop at 12 kHz is 42.7 ms. §3's median-of-5 then adds two
   hops after a step, 85 ms, which is over the ≤ 60 ms budget before I/O, snapshot or display.
   For low B: 2048-hop + median = 333 ms against 140 ms. No document breaks latency down by
   stage.
3. **The 1 kHz crossover.** At 12 kHz a 1.4 kHz period is 8.6 lags, and parabolic interpolation
   over a grid that coarse costs cents. The crossover must be measured.
4. **The full-rate path.** It needs a band-limit too: the sawtooth misses by −2.86 ¢ at 1384 Hz,
   48 kHz (`T-003a`).
5. **The gate.** A floor learned as a minimum over 2 s learns a held note as noise, and then
   mutes it.
6. **Sample rates.** Devices run at 44.1 kHz as well as 48. Coefficients and tables come from the
   rate.

Prior art: `level::RmsMeter`, the `convert` casts, `synth` fixtures, `tests/accuracy.rs` (sweep,
`report()`), `tests/no_alloc.rs` (child-process canary), `tools/bench-compare.sh`.

## Acceptance criteria

Everything is in `rust/crates/dsp`. Per-call processing allocates nothing and cannot panic,
under the deny list `pitch` uses. Every buffer and coefficient is set in a constructor.

- [ ] **`DcBlock`** is a one-pole 20 Hz high-pass, with its coefficient from the rate and its state
  in `f64`. Tests:
  - a DC step decays below −90 dB within the documented time;
  - |H(30.87 Hz)| matches its closed form;
  - silence after a signal returns the state to exactly 0 (denormals flushed).
- [ ] **`Decimator`** is a ×4 linear-phase polyphase FIR (Kaiser-windowed sinc, local Bessel I₀),
  designed per rate, with its group delay exposed. Tests:
  - stopband ≥ 80 dB and passband ripple ≤ 0.1 dB, measured with synthetic sines;
  - symmetric taps;
  - identical output for any block split (property test).
- [ ] **A band-limit low-pass on the full-rate path**, at the cutoff the probe chooses.
- [ ] **`NoiseGate`** is built on `RmsMeter` blocks, with a 2 s floor in a preallocated ring.
  - The open threshold is `max(−50 dBFS, floor + margin)`, clamped, with open/close hysteresis
    and a hold time.
  - The floor learns **only while the gate is closed**.
  - Tests: silence stays closed; a −40 dBFS tone opens it; **a note held for 10 s stays open**;
    room noise raises the threshold; a tone reopens it within the hold time.
- [ ] **`FrameAssembler`** keeps a preallocated history and yields a contiguous window every hop,
  for any input block size.
- [ ] **A range table derived per rate.** Each range has an analysis rate, a window of three
  periods of the range floor (any length; `Mpm` does not need a power of two), and a hop sized
  from the ledger.
  - A pure `RangeSelector` with hysteresis switches ranges and decimation.
  - The crossover is the highest f₀ at which the decimated path still meets ±0.5 ¢ with margin,
    measured by a recorded probe.
- [ ] **The headline gate.** The full chain (DC block → band-limit or decimate → frames → `Mpm`)
  is swept over 30–1400 Hz, at ≥ 24 points per octave and 8 phases, **at 48 and 44.1 kHz**.
  - **Sine and sawtooth must both stay within ±0.5 ¢.**
  - A failure names the frequency, range, rate, cents and clarity.
- [ ] **The latency ledger.** Feed silence, then a tone onset, through the chain. Count samples
  until the first estimate that is within ±5 ¢ and stays there. No wall clock.
  - Assert each range's count against its line in a ledger in `AUDIO_ENGINE.md` §1. The ledger
    lists I/O (provisional until `T-002f`), group delay, window fill, hop, a reserve for
    `T-003c`'s smoothing, the snapshot period and one display frame.
  - The total is ≤ 60 ms for f₀ ≥ 80 Hz and ≤ 140 ms for low B.
- [ ] `tests/no_alloc.rs` covers every new type; its canary still aborts.
- [ ] Benches `chain/decimate` and `chain/frame` are added. The baseline is re-saved on a quiet
  host. Frame cost × update rate is recorded against the 8 % CPU budget.
- [ ] **ADR 0028**: windows, hops and the crossover come from the ledger. §1 and §2 are
  rewritten to match and link it. The numbers live in code constants, and the doc names the test
  that asserts them.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

- Octave guard, median, one-euro filter and lock (`T-003c`). The ledger only reserves their
  latency.
- The note table (`T-003d`).
- Engine, FFI and UI wiring, and "the preset picks the starting range" (M3).
- Recorded fixtures (`T-003e`).

## Implementation notes

## Verification performed
