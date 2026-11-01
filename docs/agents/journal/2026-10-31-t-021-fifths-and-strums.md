# 2026-10-31 — t-021-fifths-and-strums

**Agent:** Claude Code (Opus 5.5)  **Task:** T-021  **Milestone:** M2

## Done
- **T-014 was never pushed.** STATE said "merged", but its branch had no upstream and no PR. The
  owner accepted T-014's three ceilings. Pushed, and opened as PR #42. `T-022` now makes
  `session-start` name a branch like that.
- `adr/0034`. The tests read a reading's partials (f, 2f, 3f, 5f) with `partial_near`, in the
  tail that resolves the spacing f.
  - Fifths up: a reading half the locked note is a fifth only if its upper string rings at 3f
    with ≥ ¼ of the power at 2f. It is withheld and must repeat; no needle hold.
  - From silence (`Auto` only): a reading with no fundamental and no octave is two strings struck
    together. It is withheld, shows nothing, and the selector moves as before.
  - An undecided test rejects nothing (`Analysis::undecided`).
- Gated:
  - the fifths ringing loud, at 0;
  - four strum pairs, at 0–5;
  - no fundamental from silence;
  - a note after a strum with the gate never closing;
  - High fixed by hand on odd-only and no-third timbres;
  - no gold recording held.
- `T-023` takes the fifth and unequal strums from silence.

## Tried and abandoned
- **5f to tell a fifth from one string from silence.** 5f is under the decimator's edge from
  150 Hz up, so real attacks with a weak fundamental (gold nylon D3, steel G3) read as fifths and
  blanked. **`recorded.rs` did not see it:** it judges only its window. Found by the review, with
  frame dumps.
- **"2f = 3f" when nothing resolves the fundamental:** wrong for any timbre but a sine or a
  sawtooth (a violin's pizzicato G3 with High fixed).
- **A strum latch released only by a lock:** a low B never locks, so it never showed.
- **Withholding a half reading that sounds nothing:** it blanked `ukulele_g4`'s needle.
- **Holding the needle over a fifth:** a pinch leap C3 → G2 held C3. A 1.5f test did not
  resolve.
- **Earlier, single-note costs each rule had** (`adr/0034`, Options rejected):
  - "no fundamental" alone;
  - a tail sized by k·f;
  - the onset only from a closed gate;
  - letting explained readings release `0033`'s latch;
  - withholding undecided readings below 40 Hz;
  - withholding struck readings from the selector.

## Surprises
- **`0 < ¼ × 0` is false.** A fifth with nothing at 2f or 3f latched, and a low B after an A2 went
  40 ms past its target. Only a final full audit against the baseline caught it, after every gate
  was green. The lesson: audit every line, not only the gates.
- The ukulele recordings already held 5 frames each under `T-014`: open strings ring in
  sympathy. So "no held frame" is gated on gold only.
- The review's own probes, re-run on the fixed code, were the best regression suite this task
  had. Its fourth finding, a gate dip under a fifth: 777 D2 frames → 1–2.
- A mistyped rebase base (`beb21bb`, the pre-amend commit) replayed an amended commit. Aborted
  cleanly. Use the parent of the first commit to move.

## Left for next session
- The owner: accept or reject `T-021`'s deviations (task file). Merge #42, then this PR.
- `T-022`, then `T-003d`, `T-023`.
