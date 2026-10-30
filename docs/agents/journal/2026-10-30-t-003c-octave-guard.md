# 2026-10-30 — t-003c-octave-guard

**Agent:** Claude Code (Opus 5.5)  **Task:** T-003c  **Milestone:** M2

Covers review rounds 3–5 (2026-10-24 and 10-25), which closed without a journal entry because
`/session-end` never ran `just session-end` (fixed in `T-015`), and round 6 today.

## Done
- **Reconciled.** STATE said no open PRs; Dependabot #32 (actions) and #33 (dart-minor) are open
  and green. `git fetch` failed on SSH (`publickey`) in this shell. The branch had 32 unpushed
  commits and an uncommitted round-5 fix plus ~2,150 lines of reviewer scratch probes in
  `tests/chain.rs`, which asserted nothing and ran inside `just verify`.
- **Round 3** (10-24): ringing-string rule on the selected range, `RISE`, verifier readings tested.
- **Round 4** (10-25, d009555): owner, single notes win over chord rejection. `NEW_NOTE_CLARITY`
  dropped; `AFTER_NOTE_CLARITY` binds blind estimates only; `OctaveGuard::heard`; detuned and
  low-B chords recorded, not gated. **Its docs were not updated**: `AUDIO_ENGINE.md` and ADR 0029
  still named `NEW_NOTE_CLARITY`, and §1's table was stale. Fixed today.
- **Round 5** (10-25): `Observation::Faint` replaced by `Withheld`.
- **Round 6** (today): ran the probe matrix in release at both rates, harvested it, deleted the
  scratch. One gated miss: a gap under the gate's 250 ms hold. Fixed with `gate::GateState` and
  `Observation::Holding`, test first; ADR 0029 and §1/§2 record it.

## Probe outcome matrix (round 6, before → after the hold fix)
- After-gap sweep (10 pairs × saw/sine/weak/lowb × gaps 0–0.15 s × 48 phase/offsets): 10 rows over
  a fail line → 0. Worst A2 → bass E/low B (saw) 135 → 165 ms, under the 190 ms target.
- Octave above a ringing string, 12 notes × 3 signals: 0 wrong at 10–12 dB; 6–8 dB (a chord) shows
  the lower note, 126–544 frames: recorded. 110 → 220 at 10 dB: upper note at 475–505 ms.
- Bends: 200 ¢ in 0.1 s lags 19–20 frames past 50 ¢. Vibrato ±30 ¢, repeated plucks, quiet onsets
  (−40 to −50 dBFS): 0 wrong. Onset at −50 dBFS from home Mid: low B 103 → 123 ms.
- Dense no-gap leaps: weak 73.96 → 49.31 Hz 3 wrong; 209.2 → 104.6 and 98.73 → 49.36 1 wrong.
- Gapless new note at moderate clarity: 2–8 wrong-known frames (probe judging, not the leap rule).
- Melody over a bass: shown as the held bass (by design, §3).

## Tried and abandoned
- **Treating the hold as silence** (return home): resets the guard between two plucks of a string.
- **Removing the faint rule** after the hold fix (no gated test needs it): left for `T-003f`.

## Surprises
- d009555 changed the latency of the recorded E4-ringing-loud case 60 → 115 ms with no doc change.
- A held-out gap length (0.15 s, not the gated 0.1 s) was enough to break a fail line: gates
  should sweep onset offsets, not one phase.

## Baseline before T-003c (moved here from the task file)
**Baseline**, before any change (`tests/chain.rs` at the baseline commit; 48 / 44.1 kHz, home
Mid, new note weak, partials 0.3/1/0.4). Wrong: judged clear frames off the new note, under the
leap rule. Needle: right pitch class within ±5 ¢ and stays; total adds the target's other ledger
lines (Mid 58.3, Low 63.3 ms). Before `T-003c` every frame's octave counts as known.

| Case | Wrong | Needle (ms) | Total (ms) | Octave known (ms) |
|---|---|---|---|---|
| E4 → low E, stopped | 19 / 21 | 140 / 134.9 | 198.3 / 193.3 | 140 / 134.9 |
| E4 → low E, 0.1 s gap | 0 / 0 | 27.5 / 27.3 | 85.8 / 85.7 | same |
| E4 → low E, rings (τ 0.6–3 s) | 41 / 40 | 7.5 / 1.4 | 65.8 / 59.8 | 120 / 119.7 |
| E4 → low E, 12 dB under (τ 0.6, 1.5, 3) | 44 / 46 | 125, 107.5, 107.5 / 113.6 | 183.3, 165.8 / 172.0 | 130 / 134.9 |
| A2 → bass E, stopped | 24 / 26 | 160 / 158.5 | 223.3 / 221.8 | same |
| A2 → bass E, 0.1 s gap | 0 / 0 | 50 / 53.5 | 113.3 / 116.8 | same |
| A2 → bass E, 12 dB under (τ 0.6, 1.5, 3) | 18, 19, 18 / 25, 22, 22 | 165, 185, 330 / 203.4, 163.5, 402.9 | 228.3, 248.3, 393.3 / 266.7, 226.8, 466.3 | same |
| A2 → bass E, rings (chord, τ 0.6, 1.5, 3) | 72, 199, 282 / 70, 197, 283 | 1015 / 1051; never | — | same |
| A2 → low B, stopped | 10 / 12 | 100 / 94.3 | 163.3 / 157.6 | 100 / 98.6 |

Chords show a third note in 65–270 frames (A0). Weak octave leaps down: 18–26 wrong; others 0.
## Left for next session
- Push `feat/T-003c-octave-guard` and open its PR (needs SSH in the shell, and the owner's go).
- `T-015-process-drift-4`, then `T-003f`.
