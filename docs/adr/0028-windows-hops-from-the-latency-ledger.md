# 0028 — Analysis windows, hops and the decimation crossover come from the latency ledger

**Status:** Proposed · 2026-10-23

## Context

`AUDIO_ENGINE.md` §2 tabulated windows of 2048/4096/8192 at 12 kHz with hops of 512–2048. Those
windows hold 8–27 periods (171–683 ms), against §2's own rule of three periods of the range's
lowest note. A 512 hop plus `T-003c`'s median of five is 85 ms after a step, over §1's 60 ms
before I/O or display, and no document broke the latency down by stage. The 1 kHz crossover was
never measured. `T-003a` (Deviation 2) showed that a harmonic-rich tone meets ±0.5 ¢ only when its
content stays well below the *analysis* Nyquist.

`T-003b` built the chain and measured it at 44.1 and 48 kHz. These are its findings:

1. **Where to decimate depends on the window's length in samples, not on f₀ alone.** Through the
   ×4 path, in Mid's 480/441-sample window, a 2× margin (≤ 0.25 ¢, eight phases) holds:
   - for the sine, to 604 Hz;
   - for the sawtooth, to 508 Hz at 48 kHz and to 214 Hz at 44.1 kHz.

   A three-period window of 160 Hz at the decimated rate holds 225/207 samples, and there the
   sawtooth already misses the budget at the floor (−0.82 ¢ at 160 Hz, −0.52 ¢ at 162 Hz). The
   same 18.75 ms at full rate, 900/827 samples, reads within 0.19 ¢ up to 1400 Hz.
2. **Onset latency is the window plus the filter delay**, and for the Low range a little DC-block
   settling. A partly filled window biases the NSDF (its `m(τ)` counts the silent part), so the
   first estimate within 5 ¢ waits for the window to hold only the new note.
3. **Each hop is paid three times:** once waiting for a frame, and twice more in the median of five.

## Decision

- **Three ranges, derived from the rate.** Each window holds three periods of its range's floor.
  The hops are as short as the CPU budget comfortably allows.

  | Range | Floor | Path | Window @ 48 / 44.1 kHz | Hop |
  |---|---|---|---|---|
  | Low | 30 Hz | ×4 | 1200 / 1103 (100 ms) | 5 ms |
  | Mid | 75 Hz | ×4 | 480 / 441 (40 ms) | 2.5 ms |
  | High | 160 Hz | full | 900 / 827 (18.75 ms) | 2.5 ms |

- **The crossover is the Mid/High boundary, 160 Hz,** with a semitone of hysteresis on each band.
  Its top, 169.5 Hz, sits under the tightest decimated limit (214 Hz). The decimated path cannot
  serve a window as short as High's.
- **Both paths keep their content below an eighth of their analysis rate**, where `T-003a`
  measured the parabolic bias at 0.0035 lag:
  - the ×4 anti-alias low-pass passes to fs/96 (500 Hz at 48 kHz) and stops from fs/32 (1.5 kHz);
  - the full-rate band-limit passes to 1500 Hz and stops from fs/8 (6 kHz).

  Both are Kaiser FIRs designed for 85 dB, with odd lengths so that each group delay is a whole
  number of samples.
- **The numbers live in `diapason_dsp::range`** as code constants. The ledger in `AUDIO_ENGINE.md`
  §1 restates them, and these tests assert them:
  - `range::tests` for the table;
  - `tests/chain.rs` for accuracy (`a_*_through_the_chain_*`), the crossover
    (`the_decimated_path_holds_the_budget_past_the_crossover`) and latency
    (`onset_latency_is_within_the_ledger`).
- **A pure `RangeSelector` moves between ranges:**
  - a pitch outside the current band moves to its own range;
  - silence returns to the home range;
  - a window plus 50 ms of unclear frames scans to the next range.

  Every range's history is kept full, so a switch costs one hop.

## Consequences

- §2's table and its "note" are replaced; §1 gains the ledger.
- **§1's latency budgets cannot be met as written** (`T-003b`, Deviation 1). The worst-case
  totals are 97.9 ms for High, 125.2 ms for Mid and 202.7 ms for Low, against 60 ms and 140 ms.
  I/O (20 ms, provisional), the 30 Hz snapshot (33.3 ms) and one display frame (16.7 ms) already
  sum to 70 ms before any analysis. The owner decides the remedy, for example:
  - a snapshot cadence tied to vsync;
  - a budget defined as a mean rather than a worst case;
  - revised targets.

  This ADR does not move the budgets.
- On the host, running both filters always and analysing the Low range costs about 0.4 % of one
  core. The phone figure is owed (`T-003a` owes the NSDF's too).
- A note the home range cannot see costs a scan. From Low, 1400 Hz takes 108–137 ms to reach High.
  The preset's starting range (M3) is what keeps the common case off that path.

## Options rejected

- **§2's table as written:** it breaks its own three-period rule and §1's latency.
- **A 1 kHz crossover with High decimated:** at 12 kHz High's window is 225 samples, and the
  sawtooth misses by 0.82 ¢ at its floor. The budget would fail inside the range meant to meet it.
- **The crossover at the decimated limit (214 Hz), with Mid stretched to it:** no hysteresis
  margin at 44.1 kHz, and notes from 160 to 214 Hz would wait for a 40 ms window instead of
  18.75 ms. Latency is the budget that is short.
- **Cutting the decimator at fs/4 (fewer taps, 1.3 ms less delay):** the sawtooth reached 0.51 ¢
  in the Mid window.
- **Running only the selected path's filter:** the first frame after every switch would wait for
  a window to refill. Both filters cost 0.17 % of a host core.
