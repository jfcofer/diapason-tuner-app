# 0034 — Fifths up and strums: a reading must be one string, by the partials it sounds

**Status:** Accepted · 2026-10-31 (`T-021`; the owner, 2026-10-31: from silence, show nothing
until one string reads as one)

## Context

`adr/0033` holds two strings ringing when the note locked before is 3/2, 3 or 4 times the reading,
and the reading has no fundamental. It left two families measured, not handled:
- **Fifths up** (D3 → A3, G3 → D4) are periodic at half the lower note. That is the octave guard's
  ratio (`adr/0029`), where a single string read an octave low also has no fundamental. Violin,
  viola, cello and mandolin players tune by these double stops, and the chain showed D2 or G2 as
  known in 524–547 frames of 1.5 s.
- **Two strings struck together from silence** have no locked note to suspect a reading against.
  They showed a third note in 126–547 frames, for every pair and timbre.

The owner's rule from 2026-10-25 still binds: chord rejection must not cost single notes. A phone
microphone's high-pass can remove a low string's fundamental, so "no fundamental" alone would
withhold a real low E from silence for good, which is worse than `0033`'s delay.

## Decision

- **One string explains a reading f, or the reading is two strings** (`chain::Analysis::explains`).
  The test reads the partials at f, 2f, 3f and 5f with `pitch::partial_near`, each in the shortest
  window tail that tells partials f apart (`chain::RESOLVED_BINS`). That is the same tail for
  every multiple: 3f's neighbours are 2f and 4f, only f away. One string explains f when:
  - its fundamental sounds: a string at f;
  - 2f, 3f and 5f sound, but not f: a string whose fundamental the microphone removed;
  - 2f sounds, and neither 3f nor 5f: a string at 2f read an octave low.

  What is left is two strings. A fifth (2 : 3) sounds 2f and 3f but not 5f. A fourth (3 : 4) or
  a major third (4 : 5) sounds no 2f. Two sines, read at their mean or at a quasi-period, sound
  none of them. Below what any window resolves (40 Hz in Low's, or a range fixed by hand), a
  string sounds both 2f and 3f, or neither if it is a sine. The D1 that A2 and D3 share sounds 3f
  only.
- **Fifths up** (`octave::OCTAVE_COMMON_RATIO`, `OctaveGuard::is_octave_common_period`). This is
  checked only for a reading half the locked note, while that note still rings (`adr/0033`):
  - It is a fifth if no string sounds there and the upper string sounds at 3f. It is then
    latched and held exactly as `0033`'s chords are.
  - It is no note at all if nothing sounds at f or at 3f: the locked string read an octave low
    while another beats under it, or a leap's window holding both notes. It is withheld without
    a latch, so nothing is held.
  - A real string an octave below sounds its own fundamental, and the octave guard keeps it.
- **Struck together** (`chain::Over::Nothing`). While no note has locked, every reading must be
  one string.
  - One that is not is withheld and latches the chord, so `0033`'s every-frame-repeats test
    applies until a note locks or the gate closes.
  - Nothing rang before, so `Frame::held` shows none: the owner's "show nothing until one string
    reads as one". A reading one string explains releases nothing, because either string, read
    as itself, is one.
  - The selector hears a struck reading as the pitch it read, as before this ADR. A single note
    the test misjudged therefore scans as it would have.
- **An undecided test rejects nothing** (`chain::Analysis::undecided`). Until the onset has
  passed the filters' group delay and fills the tail the test reads, that tail holds the silence
  or gap before it. The onset is the end of the block in which the gate opened, from closed or
  from its hold (`GateState::Holding` takes nothing new to be played). The rule applies to both
  struck readings and fifths: an octave-down note after a gap pays nothing.
- **This supersedes in part** `adr/0033`: its "half is the octave guard's case and stays out",
  and its "not handled" families.

## Consequences

Measured in `tests/chain.rs`, from home Mid, at both rates, diffed line by line against `T-014`:
- **Fifths up:** no third note, ringing at τ 1.5 and 3 s, for sine, sawtooth and weak (524–547
  frames before). G3 → D4 with G3 6 dB under shows none.
  - **Ceiling:** D3 → A3 with D3 6 dB under keeps 2–7 raw frames (0–5 shown) at 0.8–1.4 s. These
    are A3's octave below, A2, read before A3 locks as D3 decays. Relative to D3 they are no
    suspect, and they are never the common period (before: 274–281 D2 frames). Gated at the
    counts measured (`Gate::ChordCeiling`).
- **Strums:** no third note for B3 + E4, D3 + A3 and G3 + D4 (all timbres), A2 + D3 (sine, weak),
  D3 + G3 (weak) and G3 + B3 (sawtooth, weak).
  - **Ceilings,** in raw / shown frames: A2 + D3 sawtooth 5 / 3, D3 + G3 sawtooth 3 / 1, D3 + G3
    sine 1 / 1, G3 + B3 sine 3 / 2. These are the frames before the test can decide: the selector
    reaches Low, which shows D1 while its window still holds the silence.
  - **Held out, measured only:** the higher string 20 ms late, which is how a real strum lands,
    shows 0–4 frames, except two sines B3 + E4 at 25–26. The lower string 6 dB under shows 0–5.
- **Unchanged:**
  - every recording (`recorded.rs`, all rows);
  - every single-note latency, accuracy, leap and octave figure;
  - every `T-014` third-note count;
  - a string with no fundamental from silence (low E, A2, D3: gated, identical to `T-014`'s code).
- **Better:**
  - E4 → B3 with E4 6 dB under: 3–4 → 0;
  - A2 → low B: 150 third notes → 0;
  - 41 of `T-003c`'s detuned rows, none worse;
  - E3 → low E's needle 5 ms sooner at 48 kHz;
  - T-020's G4 with a C4 in sympathy: 324–432 → 490–532 known frames, because High's G4/2 picks
    are withheld instead of bridged ambiguous.
- **Lost:** G2 over a low B (τ 1.5 s) no longer reaches the low B, known, by 1.45 s. It still
  shows no third note.
- **Cost:** `chain/strum` 78.5 µs per 10 ms block, against 75.7 µs for `chain/analysis` and 98.5 µs
  for `chain/chord`, all on the same host run: under 1 % of a core. The test runs only before a
  note locks, or on a reading half the locked note.
- **Device risk** (re-measured on the Redmi with `T-002d`). The test assumes a string sounds 5f.
  A low string whose fundamental the microphone removed, and whose fifth partial is weak, would
  be withheld from silence. It would be delayed, never shown wrong.

## Options rejected

- **"No fundamental" alone from silence:** the tail predates the onset, which broke 11 tests,
  among them every accuracy sweep and a low E's needle (31 → 94 ms). That is why the test
  waits for the onset plus the group delay.
- **Sizing each multiple's tail to resolve k·f from 2k·f:** 3f merged with 2f, and a sawtooth
  read as a fifth.
- **The onset only from a closed gate:** after a held gap the forgotten A1 was held over a low B
  for 268 frames.
- **Letting explained readings release the latch:** after the guard forgot the old note, three
  readings of either string dropped `0033`'s latch, and D3 → G3 (sines) gained 12–14 frames.
- **A fifth by "no string explains it" alone:** a pinch leap C3 → G2 read C2, sounding nothing,
  and the old C3 was held over G2. Hence the 3f evidence.
- **Withholding undecided readings below 40 Hz:** a low B from silence took 115 ms, not 72.
- **Withholding struck readings from the selector:** a 1400 Hz note reached High a hop later.
