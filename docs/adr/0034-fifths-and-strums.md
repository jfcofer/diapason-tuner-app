# 0034 — Fifths up and strums: told from one string by the partials a reading sounds

**Status:** Accepted · 2026-10-31 (`T-021`; the owner, 2026-10-31: from silence, show nothing
until one string reads as one). Revised before merge after the review, the same day.

## Context

`adr/0033` holds two strings ringing when the note locked before is 3/2, 3 or 4 times the reading,
and the reading has no fundamental. It left two families measured, not handled:
- **Fifths up** (D3 → A3, G3 → D4) are periodic at half the lower note. That is the octave guard's
  ratio (`adr/0029`), where a single string read an octave low also has no fundamental. Violin,
  viola, cello and mandolin players tune by these double stops, and the chain showed D2 or G2 as
  known in 524–547 frames of 1.5 s.
- **Two strings struck together from silence** have no locked note to suspect a reading against.
  They showed a third note in 126–547 frames, for every pair and timbre.

The owner's rule from 2026-10-25 binds: chord rejection must not cost single notes. That covers
real recordings, a phone's high-pass, and timbres other than a sine or a sawtooth.

## Decision

The tests read partials of the reading f with `pitch::partial_near`, in the shortest tail that
tells partials f apart (`chain::RESOLVED_BINS`). That is the same tail for every multiple: 3f's
neighbours are 2f and 4f, only f away. A string sounds its fundamental. With a phone's high-pass
it still sounds 2f, 3f and 5f.
- **Fifths up** (`octave::OCTAVE_COMMON_RATIO`). This applies to a reading half the locked note,
  while that note still rings (`adr/0033`).
  - It is a fifth's common period if no string sounds there and the upper string rings at 3f,
    with at least `chain::FIFTH_UPPER_SHARE` (¼) of the power the locked string has at 2f. A
    string plucked a fifth up rings about as loud. A string in sympathy is far weaker: a ukulele's
    E4, whose octave partial is 3 × 220 Hz, under its A4.
  - A fifth is withheld, and every frame must repeat (`0033`), but the needle holds nothing. A
    leap a fourth down (C3 → G2, a pinch) reads the same while its window holds both notes, and a
    held C3 would show once the window holds only G2.
  - Anything else is left to the octave guard, as before: the locked string read an octave low,
    or a real string an octave below.
- **Struck together** (`chain::Analysis::struck`, `Over::Nothing`), with no note locked and the
  selection automatic. A reading is two strings if it sounds neither its fundamental nor its
  octave, as fourths, thirds and two sines do. Below what any window resolves (40 Hz), the sign is
  no octave but a third partial: the D1 that A2 and D3 share.
  - Such a reading is withheld, shows nothing (the owner's rule), and latches `0033`'s
    repeat test.
  - The selector hears it as the pitch it read, so a single note scans as it did.
  - Three readings in a row that no range can test at 2τ, and that one string explains, release
    it. A low B could otherwise never lock and release it.
  - **A fifth is left alone.** A string's attack with a weak fundamental sounds the same 2f and 3f
    (a nylon D3, 27 ms in). Its 5f, which would tell them apart, lies under the decimator's edge
    from 150 Hz up: −9 dB at 980 Hz, −18 dB at 1100 Hz.
  - **A range fixed by hand rejects nothing.** It has given up the ranges that could tell, and
    below 213 Hz High alone resolves no fundamental.
- **An undecided test rejects nothing** (`Analysis::undecided`). The tail holds what came before
  until the onset has passed the filters' group delay and fills it. The onset is the end of the
  block in which the gate opened, from closed or from its hold. While a chord is latched, a dip of
  the gate is no onset.
- **This supersedes in part** `adr/0033`: its "half is the octave guard's case and stays out",
  and its "not handled" families.

## Consequences

Measured in `tests/chain.rs`, from home Mid, at both rates, and diffed line by line against
`T-014`:
- **Fifths up, ringing loud** (τ 1.5 and 3 s; sine, sawtooth, weak): no third note, where there
  were 524–547 frames.
  - **Ceilings:** with the old note 6 dB under, 2–16 raw frames a case, at 0.8–1.4 s. These are
    the new note's octave below (A2, G3), read before it locks as the old string decays. They are
    never the common period, which had 274–281 frames before, and they are the octave guard's, as
    before. Gated at the counts measured (`Gate::ChordCeiling`).
- **Strums:** no third note for B3 + E4 (every timbre), A2 + D3 (sine, weak), D3 + G3 (weak) and
  G3 + B3 (sawtooth, weak).
  - **Ceilings,** raw / shown: A2 + D3 sawtooth 5 / 3; D3 + G3 sawtooth 3 / 1 and sine 1 / 1;
    G3 + B3 sine 3 / 2. These are frames before the test can decide.
  - **Not handled:** the fifths D3 + A3 and G3 + D4 still show their common period (522–545
    frames), measured only.
  - **Held out, measured only:** the higher string 20 ms late shows 0–4 frames, except two
    sines B3 + E4 at 25–26. The lower string 6 dB under shows 0–5, except with a weak
    fundamental: A2 + D3, B3 + E4 and D3 + G3 at 534–575 frames (`T-023`).
- **Single notes unchanged.** Every recording's row; every latency, accuracy, leap and octave row;
  a string with no fundamental from silence; High fixed by hand on odd-only and no-third timbres;
  a note after a strum with the gate never closing (all three gated). Two gold recordings and a
  violin's, frame by frame (nylon D3, steel G3, pizzicato G3 with High fixed). One exception,
  outside its window: on `ukulele_g4` the G an octave below shows one frame sooner.
- **Every `T-014` third-note count unchanged.** Measured only:
  - better: A2 → low B 150 → 0, E4 → B3 6 dB under 3–4 → 0, and 33 of `T-003c`'s detuned rows;
  - worse: 7 rows of E4 → low E, detuned ±30 ¢, which show 2–9 more frames on the ringing E, an
    octave off. None shows a third note.
- **Cost:** `chain/strum` 85.1 µs per 10 ms block, against 84.7 µs for `chain/analysis` and 112.4
  µs for `chain/chord`, all on one busy host run: under 1 % of a core.
- **Device risk** (re-measured on the Redmi with `T-002d`): a phone's high-pass that removes a low
  string's octave as well as its fundamental would read as a strum from silence. It would be
  withheld until it locks, never shown wrong.

## Options rejected

- **5f to tell a fifth from a string, from silence:** under the decimator's edge, it blanked 11–14
  frames of the gold nylon D3 and steel G3 during their attacks (review).
- **"No string explains it" with 2f = 3f below what resolves, under a fixed range:** High fixed
  showed a violin's pizzicato G3 on 122 frames, not 281 (review).
- **A strum's latch released only by a lock:** a low B after a strum, the gate never closing,
  was never shown (review).
- **Withholding a half reading with nothing at f and 3f:** it blanked `ukulele_g4`'s needle (2
  flicker frames). It is the octave guard's.
- **Holding the needle over a fifth:** the pinch leap C3 → G2 showed the held C3 once its window
  held only G2. So did testing for a string at 1.5f, which no tail resolves.
- **"No fundamental" alone, a tail sized to k·f, the onset only from a closed gate, and
  withholding struck readings from the selector:** each cost a single note (a low E 31 → 94 ms,
  a sawtooth read as a fifth, an A1 held over a low B, a 1400 Hz note a hop late).
