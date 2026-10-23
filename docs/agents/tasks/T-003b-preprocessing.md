---
id: T-003b
title: Pre-processing chain, measured windows and hops, and the latency ledger
status: done
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
- [x] **A band-limit low-pass on the full-rate path**, at the cutoff the probe chooses. *The
  cutoff, fs/8, came from `T-003a`'s bias figure and the sweep confirmed it; no probe chose it.*
- [ ] **`NoiseGate`** is built on `RmsMeter` blocks, with a 2 s floor in a preallocated ring.
  *All of it holds except "only while closed". Closed with #31 as Deviation 4, which the owner accepted.*
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
    measured by a recorded probe. *Deviation 2: a range boundary, chosen for latency.*
- [x] **The headline gate.** The full chain (DC block → band-limit or decimate → frames → `Mpm`)
  is swept over 30–1400 Hz, at ≥ 24 points per octave and 8 phases, **at 48 and 44.1 kHz**.
  - **Sine and sawtooth must both stay within ±0.5 ¢.**
  - A failure names the frequency, range, rate, cents and clarity.
- [x] **The latency ledger.** Feed silence, then a tone onset, through the chain. Count samples
  until the first estimate that is within ±5 ¢ and stays there. No wall clock.
  - Assert each range's count against its line in a ledger in `AUDIO_ENGINE.md` §1. The ledger
    lists I/O (provisional until `T-002f`), group delay, window fill, hop, a reserve for
    `T-003c`'s smoothing, the snapshot period and one display frame.
  - The total is ≤ 60 ms for f₀ ≥ 80 Hz and ≤ 140 ms for low B. *Moved by the owner: Dev. 1.*
- [x] `tests/no_alloc.rs` covers every new type; its canary still aborts.
- [x] Benches `chain/decimate` and `chain/frame` are added. The baseline is re-saved on a quiet
  host. Frame cost × update rate is recorded against the 8 % CPU budget.
- [x] **ADR 0028**: windows, hops and the crossover come from the ledger. §1 and §2 are
  rewritten to match and link it. The numbers live in code constants, and the doc names the test
  that asserts them.
- [x] `just verify` passes, and all six CI jobs are green (#31).

## Out of scope

- Octave guard, median, one-euro filter and lock (`T-003c`). The ledger only reserves their
  latency.
- The note table (`T-003d`).
- Engine, FFI and UI wiring, and "the preset picks the starting range" (M3).
- Recorded fixtures (`T-003e`).

## Implementation notes

By `dsp-engineer`, 2026-10-22/23, four rounds; design and numbers in `adr/0028`, `AUDIO_ENGINE.md`.
- **Step 5's common overlap** (O(1)) removed whole-period spikes (−0.514 ¢, 75.02 Hz, 44.1 kHz).
- **Selector.** A Low reference catches a range holding a low note's 2nd partial. It acts when
  fresh and two agree, or after 250 ms of agreeing streak (round 4: High stayed stuck for 10 s).
  Leap matrix (`leap.rs`, from 60 ms): round 1 26/728 cases, 92 frames; round 2 161/595; now 18/68.

**Deviation 1 — resolved by the owner.** Budgets 85/110/190 ms (fails at 120/150/260), tuner
snapshots at 60 Hz; totals 80.2/108.5/186.0 ms; Mid's headroom 1.5 / 1.1 ms (48 / 44.1 kHz).
**Deviation 2:** the crossover is a range boundary chosen for latency (the ×4 path holds a 2× margin
to ≥ 320 Hz). **Deviation 3:** a measured settling line (Low 15 ms, Mid 5, High 0).

**Deviation 4:** the gate's floor also learns while open after 2 s unpitched, not "only if closed".
A room above −50 dBFS held it open, against `PRODUCT_SPEC.md` §2; held notes still stay open.

**Deviation 5 — octave correctness across ranges moves to `T-003c-octave-guard`** (owner,
2026-10-23). The ledger is from silence; downward changes into a weak fundamental cost more
(`AUDIO_ENGINE.md` §1). Here they are bounded (settle ≤ 1 s, except chords) and kept under
ceilings at the worst below plus a margin. **Baseline**, 48 / 44.1 kHz, home Mid:

| Case | Wrong frames | First wrong (ms) | Settle (ms) |
|---|---|---|---|
| E4 → low E, stopped | 22 / 24 | 2.5 / 1.4 | 140 / 135 |
| E4 → low E, 0.1 s gap | 0 / 0 | — | 27.5 / 27.3 |
| E4 → low E, E4 rings (τ 0.6, 1.5, 3 s) | 48 / 48 | 2.5 / 1.4 | 120 / 120 |
| A2 → bass E, stopped | 35 / 38 | 2.5 / 0.3 | 160 / 159 |
| A2 → bass E, 0.1 s gap | 0 / 0 | — | 50 / 54 |
| A2 → bass E, A2 rings τ 0.6 / 1.5 / 3 s (chord) | 87, 214, 297 / 85, 212, 298 | 2.5 / 0.3 | 1015 / 1051; > 1500 |
| No-gap leap, weak, 82.41 → 41.2 / 196 → 98 Hz | 24, 22 / 26, 18 (leap rule) | — | — |

## Verification performed

2026-10-23, Fedora 44 host (Ryzen 7 8840HS), rustc 1.98.0. Host only; no device.
- `just verify` passed: 148 nextest tests (98 in `dsp`), 29 doctests.
- **Headline** (24/octave + whole-period ±0.3 Hz, 8 phases): saw 0.087 ¢, sine 0.036 ¢. Filters:
  stopband ≥ 85.40 dB (×4), 83.71 dB (band-limit), from the taps; ripple ≤ 0.001 dB.
- **Latency** vs chain line: Low 117.1/123.0, Mid 49.1/50.5, High 21.7/21.9 ms; from either home,
  160–169 Hz ≤ 80.1 ms in all (85), 80 Hz ≤ 103.2 ms (110). Loud room closes in 2.26–3.11 s.
- **Selector:** no-gap leaps (sine, saw) 0 wrong frames, ≤ 1 switch. Stuck patterns A and B leave
  at 252.5 ms. Reviewer's `stuck.rs` ringing weak low E: 500 / 628 ms before, 280 / 285 after.
- **Benches** (AC, quiet): decimate 8.67, band_limit 6.61 µs per 10 ms; frame low 11.76, mid 2.71,
  high 5.85 µs per hop; nsdf 13.0/28.7/59.7 µs. Worst CPU 0.62 % of a host core; phone owed.
- **Mutations** each fail a test. Round 4: no patience (A, B never leave); no Low reference (trap
  cases stuck); no rewind; a dropped doc column. Round 3: stale reference, held-reading end ignored,
  downward at once, no agreement, uncapped k, Mid to 169.5 Hz, upward scan, a doc cell or
  `IO_SECONDS`. Rounds 1–2: step 5, decay, learning, hysteresis, NaN, band-limit, `vec!`, bench.
