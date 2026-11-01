---
id: T-021
title: Fifths up and two strings struck together show no third note
status: done
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

`T-014` (`adr/0033`) holds the fourths, the major thirds, and the fourths and fifths down. Two
families it measured still show a third note. **Fifths up** (D3 → A3, G3 → D4): violin, viola,
cello and mandolin players tune by double stops in fifths. **Two strings struck together from
silence**, as a strum across them. Both are common enough to fix before M3 puts a needle on
screen.

## Context

Read first: `docs/adr/0033` (the rule and its rejected options), `docs/AUDIO_ENGINE.md` §3 (two
strings ringing, the octave guard), `docs/adr/0029` (the octave guard, which owns k = 2).

Measured in `T-014` (`tests/chain.rs`, `two_strings_ringing_show_no_third_note` and
`two_strings_struck_together_from_silence_are_measured`, sawtooth, from home Mid):
- D3 → A3 and G3 → D4, ringing at τ 1.5 s: 524–547 third-note frames, known. Their common period
  is half the lower note (D2, G2), the octave of the locked note: `adr/0029`'s domain, outside
  `octave::COMMON_RATIOS`. At 6 dB under, D3 → A3 is 274–281 and G3 → D4 0–4.
- Struck together: 126–547 frames for every pair and timbre. No note is locked, so no reading is
  suspected. The same spectral test may apply to any reading below both strings, but it must not
  cost single notes (owner, 2026-10-25), and a phone microphone may remove low fundamentals.

Also open: while two strings ring loud, the new note waits until the old one decays (more than
1.5 s for sines and at τ 3 s). Whether the product wants that shorter is the owner's question.

## Acceptance criteria

- [x] `just verify` passes, and all six CI jobs are green. Local `just verify` is green; CI runs on
  the PR.
- [x] *(with deviation: ceilings at 6 dB under, accepted by the owner 2026-10-31)* D3 → A3 and G3 → D4 (sine,
  sawtooth, weak; τ 1.5 and 3 s, 6 dB under; both rates) show no third note, by `T-014`'s rule
  (`Traps::third`). Ringing loud: none (524–547 before). With the old note 6 dB under: 2–16 frames a
  case, gated at their counts.
- [x] *(with deviation: two pairs not handled, accepted by the owner 2026-10-31)* The six strum pairs show no third
  note, or a measured ceiling the owner accepts. Four pairs are at 0 or a ceiling of 1–5 frames.
  The two fifths (D3 + A3, G3 + D4) are not handled (522–545) and are measured only (`T-023`).
- [x] Every `T-003c`, `T-014` and `recorded.rs` figure is unchanged, or the change is recorded
  (below, and `adr/0034`).

## Out of scope

- Showing both notes (polyphonic tuning). The engine, the FFI and the UI (M3).

## Implementation notes

`adr/0034`. The tests read partials of the reading f with `pitch::partial_near`, in the tail that
tells partials f apart (`Analysis::tail`).
- **Fifths up.** The reading is half the locked note (`OCTAVE_COMMON_RATIO`), and no string sounds
  there. It is a fifth if the upper string rings at 3f with at least `FIFTH_UPPER_SHARE` of the
  locked string's power at 2f. A fifth is withheld and must repeat, but the needle holds nothing.
  Anything else stays the octave guard's.
- **Struck together from silence** (`Analysis::struck`, `Over::Nothing`). A reading is two
  strings if it sounds no fundamental and no octave (below 40 Hz: no octave, but 3f). This applies
  under `Auto` only. Such a reading is withheld and shows nothing, while the selector moves as
  before. A latch that no reading could ever release by locking releases on three readings one
  string explains.
- **An undecided test rejects nothing** (`Analysis::undecided`): until the onset, from closed or
  from the hold, plus the group delay, fills the tail.

**The review** (`reviewer`) found three ways a single note paid, each measured on its own probes:
- From silence, 5f told a fifth from a weak-fundamental attack. 5f lies under the decimator's
  edge, and two gold recordings blanked 11–14 frames.
- `Fixed(High)` rejected odd-only timbres (a violin's pizzicato G3: 122 of 281 frames).
- After a strum, a low B never locked to release the latch, so it was never shown.

All three are fixed, and each is now gated. Its probes, re-run on the final code, are identical
to `T-014`'s, except a gate dip under a held fifth, which went from 777 D2 frames to 1–2. A last
audit found one more: a fifth with nothing at 2f or 3f latched (`0 < ¼ × 0`), which made a low B
after an A2 40 ms late (216.8 ms against a 190 ms target). Fixed: 176.9 ms, as before.

**Deviations** (all accepted by the owner, 2026-10-31):
- The fifths up with the old note 6 dB under keep 2–16 frames of the new note's octave below (A2,
  G3), read before it locks. They are the octave guard's, as before. Withholding them blanked
  `ukulele_g4`'s needle. Gated as `Gate::ChordCeiling`.
- Fifth strums from silence are not handled. From there, a fifth sounds like a string's attack
  with a weak fundamental. Measured only, with the unequal and staggered held-out strums: `T-023`.
- `FIFTH_UPPER_SHARE` (¼) is a new threshold, from the physics (a fifth's string rings about as
  loud) and one recording (`ukulele_a4`'s sympathetic E4). It was not tuned on a sweep.
- Changed figures, all measured only:
  - `ukulele_g4`, outside its window: the G an octave below shows one frame sooner.
  - A2 → low B and E4 → B3 (6 dB under) lose every third note.
  - 33 detuned `T-003c` rows are better. 7 rows of E4 → low E (±30 ¢) show 2–9 more frames on the
    ringing E, an octave off, and no third note.
  - Some held chords show the held note on fewer frames.

## Verification performed

- **Baseline** from a clean worktree of `5e55225`: every report line of `tests/chain.rs` and
  `recorded.rs` (release, both rates), diffed line by line after each change.
- **Final:**
  - every recording row and every single-note latency, accuracy, leap and octave row is
    identical;
  - the recordings are identical frame by frame (nylon D3, steel G3, pizzicato G3 with High
    fixed);
  - every `T-014` third-note count is unchanged.
- **New gates:** a string with no fundamental from silence (identical to `T-014`'s code); a note
  after a strum with the gate never closing; High fixed by hand on odd-only and no-third timbres;
  no gold recording held. Plus a no-alloc strum stream, and the `chain/strum` bench (85.1 µs per
  10 ms block, against `chain/analysis` at 84.7 µs, on a busy host).
- `just lint-rust` (pedantic, Android too) is clean. `just test-rust`: 224 passed.
