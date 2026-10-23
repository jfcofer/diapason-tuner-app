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

- [x] **`DcBlock`** is a one-pole 20 Hz high-pass, with its coefficient from the rate and its state
  in `f64`. Tests:
  - a DC step decays below −90 dB within the documented time;
  - |H(30.87 Hz)| matches its closed form;
  - silence after a signal returns the state to exactly 0 (denormals flushed).
- [x] **`Decimator`** is a ×4 linear-phase polyphase FIR (Kaiser-windowed sinc, local Bessel I₀),
  designed per rate, with its group delay exposed. Tests:
  - stopband ≥ 80 dB and passband ripple ≤ 0.1 dB, measured with synthetic sines;
  - symmetric taps;
  - identical output for any block split (property test).
- [x] **A band-limit low-pass on the full-rate path**, at the cutoff the probe chooses.
- [x] **`NoiseGate`** is built on `RmsMeter` blocks, with a 2 s floor in a preallocated ring.
  - The open threshold is `max(−50 dBFS, floor + margin)`, clamped, with open/close hysteresis
    and a hold time.
  - The floor learns **only while the gate is closed**.
  - Tests: silence stays closed; a −40 dBFS tone opens it; **a note held for 10 s stays open**;
    room noise raises the threshold; a tone reopens it within the hold time.
- [x] **`FrameAssembler`** keeps a preallocated history and yields a contiguous window every hop,
  for any input block size.
- [x] **A range table derived per rate.** Each range has an analysis rate, a window of three
  periods of the range floor (any length; `Mpm` does not need a power of two), and a hop sized
  from the ledger.
  - A pure `RangeSelector` with hysteresis switches ranges and decimation.
  - The crossover is the highest f₀ at which the decimated path still meets ±0.5 ¢ with margin,
    measured by a recorded probe. *Deviation 2: a range boundary, under the measured limit.*
- [x] **The headline gate.** The full chain (DC block → band-limit or decimate → frames → `Mpm`)
  is swept over 30–1400 Hz, at ≥ 24 points per octave and 8 phases, **at 48 and 44.1 kHz**.
  - **Sine and sawtooth must both stay within ±0.5 ¢.**
  - A failure names the frequency, range, rate, cents and clarity.
- [ ] **The latency ledger.** Feed silence, then a tone onset, through the chain. Count samples
  until the first estimate that is within ±5 ¢ and stays there. No wall clock.
  - Assert each range's count against its line in a ledger in `AUDIO_ENGINE.md` §1. The ledger
    lists I/O (provisional until `T-002f`), group delay, window fill, hop, a reserve for
    `T-003c`'s smoothing, the snapshot period and one display frame.
  - The total is ≤ 60 ms for f₀ ≥ 80 Hz and ≤ 140 ms for low B. **Not met: Deviation 1.**
- [x] `tests/no_alloc.rs` covers every new type; its canary still aborts.
- [x] Benches `chain/decimate` and `chain/frame` are added. The baseline is re-saved on a quiet
  host. Frame cost × update rate is recorded against the 8 % CPU budget.
- [x] **ADR 0028**: windows, hops and the crossover come from the ledger. §1 and §2 are
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

Built by the `dsp-engineer` agent, 2026-10-22/23. New modules: `filter` (`DcBlock`, Kaiser
`Decimator`, local I₀), `gate`, `frame` and `range` (table, `RangeSelector`, ledger constants).
The chain wiring the tests stream through is `tests/common`; the thread split is the engine's (M3).
- **Filters:** 85 dB Kaiser designs, odd taps, folded on symmetry. ×4: pass fs/96, stop fs/32,
  259 taps (GD 129). Band-limit: pass 1500 Hz, stop fs/8, 59/61 taps (GD 29/30).
- **Ranges** (`adr/0028`): Low 30 Hz ×4 (1200/1103), Mid 75 Hz ×4 (480/441), High 160 Hz full rate
  (900/827), at 48/44.1 kHz; hops 5, 2.5, 2.5 ms; semitone hysteresis. Silence returns home. A
  window plus 50 ms of unclear frames scans on, so 1400 Hz from Low takes 108–137 ms.
- **Gate:** 10 ms blocks; open at `clamp(max(−50, floor + 10), −50, −20)` dBFS; 6 dB hysteresis,
  250 ms hold. It reports and never mutes. A room louder than −50 dBFS keeps it open; clarity
  still rejects the noise.

**Deviation 1: §1's latency budgets cannot be met, and the ledger says by how much.** Worst-case
totals are High 97.9, Mid 125.2 and Low 202.7 ms, against 60 (f₀ ≥ 80 Hz) and 140 ms (low B).
I/O 20 (provisional) + snapshot 33.3 + display 16.7 = 70 ms before any analysis. The chain lines
are met (below), the totals are not; that criterion stays unticked, and the budgets are the owner's
call (snapshot cadence, mean vs worst case, or targets). ADR 0028 is **Proposed** until then.

**Deviation 2: the crossover is a range boundary, under the decimated limit.** The recorded probe
(≤ 0.25 ¢, eight phases) holds ×4 in Mid's 480/441-sample window: sine to 604 Hz, sawtooth to 508
(48 kHz) and 214 Hz (44.1 kHz). In a 160 Hz three-period window at fs/4 (225/207) the sawtooth
misses the *budget* at 160 Hz (−0.82 ¢, dense probe); at full rate (900/827) ≤ 0.19 ¢ to 1400 Hz.
So 160 Hz (top 169.5) it is: at 214 Hz, 160–214 Hz notes would wait 40 ms instead of 18.75.

**Deviation 3: the ledger adds a measured settling line** (Low 15, Mid 5, High 1 ms): the DC
block's onset transient keeps low B past 5 ¢ for ~12 ms after the window fills.

## Verification performed

2026-10-23, Fedora 44 host (Ryzen 7 8840HS, **on battery, powersave**), rustc 1.98.0. Host only.
- `just verify` passed: 126 nextest tests (76 in `dsp`; slowest 20 s in debug), 27 doctests (22).
- Headline worst |error|, 24 points per octave × 8 phases, every range holding each f₀:
  sine 0.057 ¢ (48 kHz), 0.060 ¢ (44.1); sawtooth 0.250 ¢ (48, Mid, 71.4 Hz), 0.278 ¢ (44.1, Mid,
  75.6 Hz); Low ≤ 0.147, High ≤ 0.187.
- Filters, with sines: stopband ≥ 85.7 dB (×4), ≥ 83.7 dB (band-limit); ripple ≤ 0.0063 dB.
- Onset latency, worst of both rates, against the chain line: Low 117.1/123.0 ms (sine, 30.87 Hz),
  Mid 49.1/50.5 (sine, 75 Hz), High 21.3/22.9.
- Bench medians: decimate 9.49 µs and band_limit 7.31 µs per 10 ms block; frame low 12.8, mid
  2.92, high 6.36 µs per hop; nsdf 14.1/30.0/63.9 µs. That is 0.42 % of one host core at worst
  (both filters plus Low or High), against 8 %. **The phone figure is owed.** nsdf reads about 8 %
  above `T-003a`'s AC baseline with no code change; re-save on AC.
- Each gate was proven by breaking it. Band-limit bypassed: sawtooth −2.52 ¢ (44.1 kHz, 1141.6 Hz)
  and +2.73 ¢ (48 kHz, 1400 Hz). A 0.6 ¢ detune fails the sweep (−0.65 ¢). A floor that learns
  while open closes the held note at 2.26 s. Zero settling fails the latency test (117.1 against
  108.0 ms). A `vec!` in `Decimator::process_sample` aborts both new no_alloc tests. A doubled
  convolution fails `just bench` (+124 %).
