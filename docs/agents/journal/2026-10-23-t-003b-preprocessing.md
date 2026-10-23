# 2026-10-23 — t-003b-preprocessing

**Agent:** Claude Code (Opus 5.5), with `dsp-engineer` and `reviewer` subagents  **Task:** T-003b,
T-003c (plan)  **Milestone:** M2

Started 2026-10-22 in plan mode; ran past midnight.

## Done
- **Reconciled.** #30 had merged; STATE still called it open. Closed T-003a. The first
  `git fetch` failed on SSH (`publickey`), and a retry worked; local `main` matched `origin/main`.
- **Planned T-003b**, after finding six places where `AUDIO_ENGINE.md` §2 contradicts §1:
  windows far above the three-period rule, hops plus median-of-5 over the latency budget, an
  unmeasured 1 kHz crossover, no band-limit at full rate, a gate that would learn a held note, and
  rates fixed at 48 kHz.
- **Built it** in four implementation rounds, each followed by an adversarial review:
  - `dsp::{filter, gate, frame, range, chain}`;
  - a common-overlap step 5 in `Mpm`;
  - a latency ledger in §1, which a test parses from the doc;
  - ADR 0028.
  
  Results: ±0.087 ¢ sine and sawtooth over 30–1400 Hz at 44.1 and 48 kHz, with host CPU around
  0.6 %.
- **Owner decisions:**
  - (1) §1's budgets were infeasible on a worst-case count (I/O, snapshot and display come to
    70 ms before any analysis). New budgets: ≥ 160 Hz ≤ 85 ms, 80–160 Hz ≤ 110 ms, low B ≤ 190 ms,
    with tuner snapshots at 60 Hz (ADR 0028). AGENTS.md, ARCHITECTURE, GLOSSARY and PLATFORM_AUDIO
    now say so.
  - (2) Octave correctness across ranges moved to `T-003c-octave-guard`; smoothing and lock split
    out as `T-003f`.

## Tried and abandoned
- **Fixing the sawtooth's integer-period spikes by changing window lengths.** That only moves
  them, to k·rate/N. The cause is step 5's three NSDF samples summing different overlaps, and the
  fix is a common overlap computed in O(1).
- **Three range-selector heuristics, each traded for another octave error:**
  - round 1: High locked onto 2f₀ for low notes it cannot see;
  - round 2, with a Low reference: gapless note changes read a common sub-period (161 of 364 cases);
  - round 3, with freshness and agreement: a ringing previous string held the wrong octave for up
    to 655 ms, and some patterns never left.
  
  Round 4 added only a hard 250 ms patience bound. The real fix belongs with the estimate-level
  octave guard (T-003c).
- **Gate floor learning "only while closed"** (the original criterion): a room louder than
  −50 dBFS kept it open for good. It now also learns after 2 s with no pitch (Deviation 4).

## Surprises
- The test grid (24 points per octave) stepped over narrow error spikes: −0.514 ¢ at 75.02 Hz,
  44.1 kHz. Dense probes around k·rate/N are now part of the sweep.
- Each review round found something the previous round's tests could not see. Counting wrong
  frames, rather than only settle time, is what exposed the octave trade-offs.
- The bench baseline was first saved on battery (+5–8 %). It was re-saved on AC. Even idle, the
  10 % gate failed one µs-scale entry in 2 of 4 runs.
- ADR 0028 was edited after being marked Accepted, but all within this unmerged PR.

## Left for next session
- Push `feat/T-003b-preprocessing`, open the PR, get six green checks; the owner rebase-merges;
  then close T-003b.
- `T-003c-octave-guard`, starting from T-003b's measured octave-trap baseline.
- Consider a dev-profile `opt-level` for `realfft`/`rustfft` (`dsp` tests take ~50 s in debug),
  and a less noisy bench gate.
