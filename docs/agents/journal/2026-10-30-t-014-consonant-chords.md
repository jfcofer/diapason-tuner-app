# 2026-10-30 — t-014-consonant-chords

**Agent:** Claude Code (Opus 5.5)  **Task:** T-014  **Milestone:** M2

## Done
- `adr/0033`: a common period has no fundamental of its own, and a string does.
  - `pitch::partial_near`: a Hann DFT on a ±150 ¢ grid.
  - The chain suspects a reading 3/2, 3 or 4 times below the note locked before
    (`OctaveGuard::anchor`, `octave::COMMON_RATIOS`), while that note still rings. It then tests
    the reading's own peak in the shortest window tail that resolves it.
  - A failing suspect is withheld. Until another note locks, frames must repeat at 2τ, and the
    needle holds, flagged ambiguous (`Frame::held`).
- Gated: the three fourths (3 timbres, τ 1.5/3 s, 6 dB under, both rates), the five loud chords
  of `T-003c`'s review, and E3 over a low E. Measured: fifths, thirds, downward intervals,
  held-out detunings and strums. `chain/chord` bench; a no-alloc chord stream.
- Owner's scope (asked at the start): gate the fourths; measure the rest. `T-021` holds what fails.

## Tried and abandoned
- **Onset detection first** (the task's own hint). An onset says when, not what: a real low E
  under its E4 and the chord B3 + E4 both start with one and read the same E2. Never built.
- **`locked` as the reference:** it follows each frame of its run, and D3's attack dragged A2 to
  117 Hz (+108 ¢) in five frames, so D1 was no longer a third of it. Hence `anchor`.
- **Ratios 3 and 4 only:** weak-fundamental A2 + D3 read D2 (A2's octave partial with D3, a
  fifth). Adding 3/2 fixed it, then broke the leap sweep's fifth-down leaps until the rule also
  required the old string to still ring.
- **The fundamental's peak in Low's whole window:** 50–150 ¢ flat for 70 ms after the onset (the
  window held the old note), which withheld every `E4 → low E` gate. The shortest resolving tail
  fixed it. The *anchor's* presence needs the opposite, the whole window, or B3 and E4 merge.
- **Holding the needle's last value:** it froze on attack blends and quasi-periods. It now holds
  the last value it showed *of the anchor*.
- **Holding from the first suspect:** detuned E4 → low E (12 dB under) showed the held E4 for
  30 ms, before the low E's fundamental could resolve. The hold now waits that long.
- **A release on one rescue:** C3 over a bass E rescued falsely once, and the chord came back.
  Three in a row.
- **`AFTER_NOTE_CLARITY` 0.88** for D3 + G3's 0.872 frame: one rate only, and a recording broke.

## Surprises
- Pure sines are the hard case. Two sines read their mean frequency (174.5 Hz for D3 + G3) or
  quasi-periods, and a sine 10 dB under bends High's estimate of the louder one by ±65 ¢ at the
  beat rate. Harmonic signals make the mixture periodic, which the rule handles.
- The task said E3 over a low E showed E3 known; measured from home Mid, no judged frame was wrong
  on `main` already. Gated as is.
- My first summary filter skipped the needle's columns and hid three residuals. Always parse every
  column of a report before claiming a gate passes.
- `dsp`'s clippy list bans `Vec::push` in tests too (the RT list). Added to STATE's trap.
- **The review found a latch bug no gate covered:** another note locking left the chord latch
  set, so the old note, plucked again, was held at once. Fixed (`release_chord`), now gated. Its
  fix first cleared everything when the old string seemed to stop too, which brought back G2 over
  a low B: that presence test flickers, so it drops the latch only.

## Left for next session
- `T-021` (active), then `T-003d` (note table). The owner: accept or reject the three ceilings
  (`Gate::ChordCeiling`, `adr/0033`) and the 2.5 ms on E4 → low E at 44.1 kHz; decide whether a
  loud chord should hold the old note for over 1.5 s (STATE, Open questions).
