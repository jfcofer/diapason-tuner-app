# 0033 — Two strings ringing: a common period has no fundamental, and the needle holds

**Status:** Accepted · 2026-10-30 (`T-014`)

## Context

`PRODUCT_SPEC.md` §2: "Two strings ringing at once: hold the last confident estimate rather than
alternating." A consonant interval is periodic at the two notes' common period, which no
periodicity test can tell from one note (`adr/0029`, `0031`). From home Mid, A2 + D3 read D1,
D3 + G3 read G1, and B3 + E4 read E2, at clarity 1.0. This happens because the range playing
reads nothing clear, and a verifier below it reads the common period. The current range cannot
see that period, and the held note is 3 or 4 times it, so `RangeSelector::observe_verifier` takes
it for a hidden fundamental and moves the chain down. The lower range then shows it as known.
Loud non-consonant pairs read the old note's sub-harmonic instead: G2 over a low B, C3 over a bass
E, and A2 detuned +60 to +100 ¢ over a bass E. A real low string can also sit exactly there. A
low E rings under the E4 it is a quarter of (`T-003c`'s gated `E4 → low E` cases), and a low E is
a third of a B3.

The task also asks which layer owns "hold": the guard or the needle (`adr/0032`). And it asks how
the tuner knows which string was just plucked.

## Decision

- **A common period has no fundamental of its own; a string does.** Two strings whose common
  period is f put energy only on the multiples of f that are partials of one string or the other.
  None of it falls at f, where a string at f sounds, however weak its fundamental. That is what
  separates the cases above, so no time-based onset model is needed. The chain does not need to
  know which string was just plucked. It needs to know whether a reading is one string.
- **Suspect, then test** (`chain::Analysis`, every reading of the selected range and of the
  verifiers):
  - *Suspect.* A note has locked, and its value as it locked (`OctaveGuard::anchor`, which a
    second string's attack cannot drag) is 3/2, 3 or 4 times the reading, within 50 ¢
    (`octave::COMMON_RATIOS`). Two thirds covers a weak-fundamental A2's octave partial and D3,
    which read D2. Half is the octave guard's case and stays out.
  - *The old string still rings.* Its partial is found in the reading range's whole window,
    `pitch::partial_near`. Otherwise a note left for one a fifth below is no chord.
  - *The test.* `pitch::partial_near` runs a Hann-windowed DFT on a ±150 ¢ grid, refined by a
    parabola in decibels. It runs on the shortest recent tail of the window that resolves the
    reading from its octave (`chain::RESOLVED_BINS`, four bins: 40 Hz in Low's window), so little
    of the tail predates the onset. A reading is a string if a peak lies within 50 ¢ of it with at
    least `chain::FUNDAMENTAL_SHARE` (10⁻³) of the frame's power. Below 40 Hz no window
    resolves it, and the reading is never taken for a string while it is suspect.
- **A suspect that fails is the chord.** It is withheld (`Observation::Withheld`, no miss), its
  verifier moves nothing, and the old note counts as heard, so it is not forgotten.
  - *Every frame must be one string.* Until another note locks, even after the guard forgets the
    old one, whatever is shown must be the held note, or must repeat at twice its period (`REPEATS`). The
    test runs in the first range, from the selected one down, that can search 2τ there. Two strings
    also read as quasi-periods of neither (two sines: 131.8 Hz, or their mean, 174.5 Hz), and an
    old sine beating under the new one bends High's reading by up to 65 ¢. Neither repeats.
  - *Release.* Another note locking, three suspects in a row with a fundamental, or the gate
    closing release it; the old string stopping drops the latch (`the_hold_of_two_strings_lets_go`).
- **The detector decides; the needle holds.** `Frame::held` is set while the strings ring and the
  frame has no estimate. Such a frame shows the needle's last value of the held note, flagged
  `ambiguous octave`, and does not touch the needle's history. While two strings ring the octave
  is not decided: a low E under its E4 is the same letter. The estimate (`frequency_raw`) stays
  `None`, so the lock and the strobe never run on a stale value. The hold starts only once a
  suspect has failed for longer than its fundamental takes to resolve. Before that, a low string
  just plucked may simply not be resolved yet, and frames are withheld without a hold. M3 decides
  how a held needle looks.
- **This supersedes in part** `adr/0029`: its known gap on loud chords, for the families above.
  It also supersedes `adr/0031`: a verifier's third or quarter now moves the chain only if it is no
  chord. And it supersedes `adr/0032`: "a frame without an estimate shows none" holds except for a
  `held` frame.

## Consequences

- **Measured** (`tests/chain.rs`, `two_strings_ringing_show_no_third_note`, from home Mid, at both
  rates): the three fourths show no third note on the estimate or the needle. That holds for sine,
  sawtooth and weak fundamental, ringing at τ 1.5 and 3 s and 6 dB under, all except three
  residuals below (before: 113–573 frames each). G2 over a low B, C3 over a bass E, and A2 +60, +80
  and +100 ¢ over a bass E show none. E3 over a low E shows no wrong frame. On held-out rows
  (fourths detuned ±7, ±22 and ±45 ¢, fourths and fifths down, major thirds up), there is no
  third note except E4 → B3 at 6 dB under: 3–4 frames.
- **Three residuals, gated at their measured counts** (`Gate::ChordCeiling`):
  - D3 + G3, two sines, D3 6 dB under: one estimate from High 21 ms after the onset, before any
    range reads the common period.
  - A2 + D3, two sines, A2 6 dB under: the needle 60 ¢ behind the new note's converging estimate
    for up to two frames, as the judge starts at 47.5 ms.
  - C3 over a bass E, τ 1.5 s, 44.1 kHz: one estimate 51 ¢ sharp of the bass E at 1.39 s, after
    it locked. The reading is biased by the C3 still ringing; it is no common period.
- **Unchanged:** every recording (`recorded.rs`, all 20 rows, line by line), the leap sweep, the
  octave-above and pinch gates, and every latency from silence. T-020's G4 over G4/3 settles
  sooner (348 → 432 known frames). The detuned A2 → bass E chords of `T-003c`, recorded only, lose
  every third note: they had up to 96 frames.
- **Cost:** E4 → low E, 12 dB under, at 44.1 kHz: the low E is a quarter of the E4 and waits one
  hop for its fundamental to resolve, so the needle takes 61.3 ms, not 58.7, and the total is
  114.6 ms against a 150 ms fail line (target 110). At 48 kHz nothing changed. CPU: a chord costs
  `chain/chord` 105 µs per 10 ms block, against 80 µs for `chain/analysis` on a single note: 1.05 %
  of a host core, measured on a busy host.
- **While two strings ring, the new note waits.** The held note shows until the old string has
  decayed enough for the new one to read as one string. For the gated fourths, with the old note
  6 dB under and τ 1.5 s, that takes 1.1–1.3 s for a sawtooth and 40–65 ms with a weak
  fundamental. Sines, every τ 3 s case and every loud case take longer than the 1.5 s measured.
  That is the specification's "hold". Showing the new note through the chord would be polyphonic
  tuning, a product decision of its own, and the owner may want it shorter (`T-021`).
- **Not handled, measured (`T-021`):** fifths up (D3 → A3, G3 → D4), whose common period is half
  the lower note, the octave guard's case; and two strings struck together from silence, where no
  note has locked to suspect a reading against. Both still show a third note.
- **Device risk:** a phone microphone's high-pass can remove a low fundamental. The test then
  fails, and a real low string at a sub-harmonic of the one ringing is *held* until the other
  decays. It is delayed, never shown wrong. It is re-measured on the Redmi with `T-002d`.

## Options rejected

- **Onset detection** (spectral flux: Bello et al. 2005, Böck & Widmer 2013), `T-014`'s first
  idea. An onset says *when* a string was plucked, not *what* the mixture is. A real low E under
  its E4 and the chord B3 + E4 both start with an onset and read the same E2. It would add a
  detector, its thresholds, and false onsets on bowing and vibrato, for no case the spectrum
  leaves open.
- **Peak position over Low's whole window:** a whole window spanning the onset put the low E's
  fundamental 50–150 ¢ flat for 70 ms, which withheld every `E4 → low E` gate.
- **No test that the old string still rings:** a fifth-down leap (sine, 130.81 → 87.21 Hz) held
  the old note for 6–39 frames. Testing it in the shortest tail merged B3's partial with E4's.
- **Holding the needle's last value, whatever it was:** it froze on an attack blend (A2 dragged to
  116.5 Hz) or a quasi-period (50.9 Hz). Exempting the *locked* note, which creeps, from the
  repeat test let the beating G3 through at 65 ¢; and without the latch, so did the old sine's
  beating once no range read the common period any more.
- **`AFTER_NOTE_CLARITY` 0.88** (for D3 + G3's 0.872 frame): it fixed one rate only and broke a
  recording.
