# 2026-10-30 — t-020-ukulele-g4-tritave

**Agent:** Claude Code (Opus 5.5)  **Task:** T-020  **Milestone:** M2

## Done
- Traced `ukulele_g4` frame by frame. The chain left High through the verifier, not a miss scan.
  At 1.200 s Mid read 130.7 Hz, G4/3, and `harmonic` accepted k = 3. An FFT showed why Mid read
  it: a body tone at 134.8 Hz (−11 dB re G4) and a sympathetic C4 (−21 dB).
- Fix: at a third or a quarter of the current pitch, a verifier moves the chain down only to a
  pitch the current range cannot see (`adr/0031`). Added a `range` unit test, a 24-case chain gate
  (`over_a_third`), a weak-odd-partials gate, and a gross-frame gate on every ceiling fixture.
- Reconciled STATE: T-003e merged as #39, not #38. Dependabot #33 was closed unmerged. #37 is new.

## Tried and abandoned
- **A decaying G4 as the synthetic case** (the task's wording): once its low tone ends louder than
  about −10 dB, High itself reads G4/3. That is a chord (`T-014`), not this bug. The steady case at
  the recording's levels separates the two.
- **A guard rule after the switch:** a common period repeats at 2τ, so `REPEATS` cannot reject
  it, and once the chain is in Mid the evidence is gone (High saw 130.7 Hz and read G4).
- **Visibility for every multiple** (first draft): the `reviewer` subagent built a string with
  weak odd partials that High reads at 2f₀ though it sees f₀; the draft left 2f₀ known for the
  whole note. k = 2 is exempt now, and every pre-existing chain figure is identical to `main`.

## Surprises
- 10 `ukulele_g4` frames became `ambiguous`. That is right: High's pick flips to G4/2 at 1.250 s,
  and the τ/2 bridge shows the held G4.
- `just bench` could not get a quiet host (Chrome at 94 % of a core). Its two runs disagreed
  wildly on untouched code. CI does not run the bench; the rerun is now a `T-003f` criterion.

## Left for next session
- `T-003f`, whose criteria now include the quiet-host bench. The weak-odd 2f₀ transient
  (STATE, known gaps) has no owner yet.
