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
- [x] **A band-limit low-pass on the full-rate path**, at the cutoff the probe chooses. *The
  cutoff, fs/8, came from `T-003a`'s bias figure and the sweep confirmed it; no probe chose it.*
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
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

- Octave guard, median, one-euro filter and lock (`T-003c`). The ledger only reserves their
  latency.
- The note table (`T-003d`).
- Engine, FFI and UI wiring, and "the preset picks the starting range" (M3).
- Recorded fixtures (`T-003e`).

## Implementation notes

By the `dsp-engineer` agent, 2026-10-22/23 (round 2: owner's decision, review). Design and numbers:
`adr/0028`, `AUDIO_ENGINE.md` §1–§3, `dsp::range`.
- **Modules** `filter`, `gate`, `frame`, `range`, `chain`: `Front` (callback) passes its gate state
  across the ring to `Analysis`, which runs the rest. `tests/common` is only the engine's glue.
- **Whole-period spikes (review).** The FFT NSDF's three peak samples sum `N − τ` pairs each. A
  sawtooth's reset at the window's edge tilts them, most when the window holds whole periods
  (−0.514 ¢ at 75.02 Hz, 44.1 kHz, Mid). Step 5 now uses one common overlap, counted from both ends,
  in O(1) from the FFT's sums (`Mpm::common_overlap`). Worst in the tuner's windows: 0.50 → 0.087 ¢;
  T-003a's three-period sine: 0.104 ¢ on its grid (−0.507 ¢ dense, before).
- **Octave above (review).** High is blind below 107 Hz and Mid below 50 Hz. A Low reference frame
  per Low hop, while another range is selected, moves a range holding k·p to p's range. Misses now
  decay instead of resetting. **Loud room (review):** after 2 s open with no clear pitch, the
  gate's floor learns while open.

**Deviation 1 — resolved by the owner, 2026-10-23.** The first ledger (30 Hz snapshots) totalled
97.9/125.2/202.7 ms against 60/140 ms; I/O, snapshot and display alone were 70 ms. The owner kept a
worst-case ledger, moved tuner snapshots to 60 Hz and set the budgets to 85/110/190 ms
(fails at 120/150/260). Totals at 48 kHz are 80.2, 108.5 and 186.0 ms. Mid has 1.5 ms of headroom.

**Deviation 2 — the crossover is a range boundary chosen for latency.** In Mid's window the ×4
path keeps a 2× margin to at least 320 Hz (the probe's cap). A decimated 160 Hz three-period window
(225 samples) now reads +0.038 ¢ at 160 Hz. Before the common overlap it read −0.82 ¢ (48 kHz, 8
phases × 48 points per octave, from the uncommitted scratch probe `examples/probe.rs`). Notes above
160 Hz go to High for its 18.75 ms window. High is full rate because the decimator stops at 1.5 kHz.

**Deviation 3:** the ledger adds a measured settling line: Low 15 ms, Mid 5, High 0 (not needed).

## Verification performed

2026-10-23, Fedora 44 host (Ryzen 7 8840HS), rustc 1.98.0. Host only; no device.
- `just verify` passed: 141 nextest tests (91 in `dsp`), 29 doctests (24). `dsp`'s tests take
  ~40 s wall in debug, most of it in debug-built FFTs.
- **Headline** (24/octave plus whole-period frequencies ±0.3 Hz, 8 phases): sawtooth 0.087 ¢, sine
  0.036 ¢. **Filters:** exact minimum stopband 85.40 dB (×4), 83.71 dB (band-limit); ripple
  ≤ 0.001 dB; sines agree with the taps to 5·10⁻⁹ beyond 0.1 %.
- **Latency** against the chain line: Low 117.1/123.0, Mid 49.1/50.5, High 21.7/21.9 ms. **Octave
  guard:** low E after E4 in Mid in 48–75 ms; bass E after A2 in Low in 95–110 ms. **Loud room:**
  −45 dBFS closes in 2.26–3.11 s.
- **Benches** (whole baseline re-saved on AC, quiet): decimate 8.67 µs and band_limit 6.61 µs per
  10 ms block; frame low 11.76, mid 2.71, high 5.85 µs per hop; nsdf 13.0/28.7/59.7 µs. Two of
  four confirming runs failed one entry just over 10 % (noisy at this scale). Worst CPU: 0.62 % of
  a host core (filters, High, Low reference). **The phone figure is owed.**
- **Mutations**, each failing its test: textbook step 5 (−0.507 ¢ at 75.000 Hz); no Low reference
  (both octave tests: Mid at 81.45 Hz, High at 165.44 Hz); misses reset; no learning while open
  (gate and chain); no hysteresis; no NaN reset; `IO_SECONDS` 21 ms (the ledger table). Plus round
  1's: band-limit bypassed, detune, held note, settling, `vec!`, slowed bench.
