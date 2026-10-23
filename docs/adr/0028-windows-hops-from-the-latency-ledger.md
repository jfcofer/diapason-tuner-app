# 0028 — Analysis windows, hops and the decimation crossover come from the latency ledger

**Status:** Accepted · 2026-10-23

## Context

`AUDIO_ENGINE.md` §2 tabulated windows of 2048/4096/8192 at 12 kHz with hops of 512–2048: 8–27
periods (171–683 ms), against its own rule of three periods of the range's lowest note. A 512 hop
plus `T-003c`'s median of five is 85 ms after a step, and no document broke latency down by stage.
The 1 kHz crossover was never measured. `T-003a` showed a harmonic-rich tone meets ±0.5 ¢ only
when its content stays well below the *analysis* Nyquist.

`T-003b` built the chain and measured it at 44.1 and 48 kHz:

1. **Onset latency is the window plus the filter delay.** A partly filled window biases the NSDF
   (its `m(τ)` counts the silent part), so the first estimate within 5 ¢ waits for the window to
   hold only the new note. Low also waits ~12 ms for the DC block's transient.
2. **Each hop is paid three times:** once waiting for a frame, and twice in the median of five.
3. **Step 5's three NSDF samples come from three different sums** (`N − τ` pairs each). A sawtooth's
   reset at the window's edge tilts them, worst when the window holds whole periods: −0.514 ¢ at
   75.02 Hz (44.1 kHz, Mid), between the points of any octave grid. Recomputed over one common
   overlap, counted from both ends, the worst error in the tuner's windows is 0.087 ¢.
4. **With that, the ×4 path is accurate in short windows too.** In Mid's window it keeps a 2×
   margin (≤ 0.25 ¢) to at least 320 Hz. A decimated 160 Hz three-period window (225 samples) reads
   +0.038 ¢ at 160 Hz, where it had read −0.82 ¢ before.
5. **The first ledger could not meet §1.** With 30 Hz snapshots, I/O (20 ms, provisional), snapshot
   (33.3) and display (16.7) summed to 70 ms, over the 60 ms target, before any analysis.

## Decision

- **Three ranges, derived from the rate.** Each window holds three periods of its floor, and the
  hops are as short as the CPU budget comfortably allows.

  | Range | Floor | Path | Window @ 48 / 44.1 kHz | Hop |
  |---|---|---|---|---|
  | Low | 30 Hz | ×4 | 1200 / 1103 (100 ms) | 5 ms |
  | Mid | 75 Hz | ×4 | 480 / 441 (40 ms) | 2.5 ms |
  | High | 160 Hz | full | 900 / 827 (18.75 ms) | 2.5 ms |

- **The crossover is the Mid/High boundary, 160 Hz,** with a semitone of hysteresis, as a latency
  choice. Higher notes get High's 18.75 ms window. High runs at full rate because the decimator's
  content stops at 1.5 kHz and High reaches 1400 Hz.
- **Both paths keep their content below an eighth of their analysis rate**, through 85 dB Kaiser
  FIRs with odd lengths (whole-sample group delay):
  - the ×4 low-pass passes to fs/96 and stops from fs/32;
  - the band-limit passes to 1500 Hz and stops from fs/8.
- **Step 5 interpolates over a common overlap** (`pitch::Mpm::common_overlap`). It costs O(1): each
  sum is the FFT's full sum minus at most two edge pairs.
- **The selector:**
  - hysteresis between bands, home on silence;
  - misses that decay rather than reset;
  - a scan to the next range after a window plus 50 ms of unclear frames;
  - a **Low reference frame** while another range is selected, so that a range blind below 1.5
    periods of its floor cannot hold a low note's second partial.
- **The chain splits at the ring buffer.** `chain::Front` (DC block, gate) runs in the callback;
  its gate state crosses the ring with each block. `chain::Analysis` does the rest, and it reports
  an open gate *unpitched* after 2 s without a clear pitch. The gate's floor then learns while open.
- **The ledger stays a worst-case count, and the owner set the budgets from it** (2026-10-23):

  | Range | Target | Fails at | Ledger at 48 kHz |
  |---|---|---|---|
  | f₀ ≥ 160 Hz | ≤ 85 ms | > 120 ms | 80.2 ms |
  | 80–160 Hz | ≤ 110 ms | > 150 ms | 108.5 ms |
  | low B | ≤ 190 ms | > 260 ms | 186.0 ms |

  They replace ≤ 60 ms for f₀ ≥ 80 Hz and ≤ 140 ms for low B. **Tuner snapshots publish at 60 Hz**,
  so the snapshot line is 16.7 ms.
- **This refines `adr/0001` and `adr/0022`'s "~30 Hz snapshot", for the tuner only.** The
  metronome's cadence is undecided. Both ADRs stay as written.
- **The numbers live in `diapason_dsp::range`.** These tests assert them:
  - `range::tests`, for the table and the ledger's lines and totals against §1;
  - `tests/chain.rs`, for accuracy, the crossover, latency, the octave guard and the gate.

## Consequences

- **M3 must publish tuner snapshots at 60 Hz;** the ledger's snapshot line binds it.
- **Low power:** under `PLATFORM_AUDIO.md`'s low-power rule (tuner snapshots 60 → 20 Hz), the tuner
  trades latency for battery, and that row of the ledger does not apply.
- Mid's headroom is 1.5 ms at 48 kHz and 1.1 ms at 44.1 kHz: any added delay there (filter, hop, settling) needs a new decision.
- Host CPU, worst case, is about 0.6 % of one core: both filters, High's frames and the Low
  reference. The phone figure is owed (`T-003a` owes the NSDF's too).
- From Low, a 1400 Hz note costs a scan (108–137 ms). The preset's starting range (M3) keeps the
  common case off that path.

## Options rejected

- **§2's table as written:** it breaks its own three-period rule and §1's latency.
- **Keeping 30 Hz snapshots with looser budgets:** 16.7 ms of every range's latency goes to a
  cadence chosen for UI throttling, not for the tuner. Display-rate snapshots cost little.
- **Keeping 60 ms and redesigning:** no detector fits. I/O, snapshot and display already take
  53–70 ms, and a window shorter than about two periods of an 80 Hz note (25 ms) has no period
  for `Mpm` to find.
- **A 1 kHz crossover:** the decimator's content stops at 1.5 kHz, so the top of High cannot be
  decimated. Stretching Mid upward instead gives those notes a 40 ms window.
- **The textbook step 5:** it leaves the whole-period spikes past the budget.
- **Computing the common overlap directly:** six O(N) sums, measured at +40 µs on an 8192 frame.
- **Cutting the decimator at fs/4 (1.3 ms less delay):** the sawtooth reached 0.51 ¢ in Mid
  (before the common overlap).
- **Filtering only the selected path:** each switch would wait for a window to refill.
