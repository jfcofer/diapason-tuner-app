# 0029 — Octave guard: provisional octaves, verified by the ranges below

**Status:** Accepted · 2026-10-23 (the ambiguity rule decided by the owner, 2026-10-23). Revised
before merge, on `T-003c`'s branch, after review rounds 3 to 6 (2026-10-24 to 2026-10-30; the
owner's decision that single notes win over chord rejection, 2026-10-25); the decision date stands.

## Context

`T-003b` (`adr/0028`) left octave correctness across ranges open. `Mpm` searches lags up to half its
window, so a range cannot see below 1.5 periods of its floor (High 106.9 Hz, Mid 50.2 Hz), and a low
note with a weak fundamental, heard by a range above it, reads as its second partial. Measured
before (home Mid, 48 / 44.1 kHz): E4 → low E, stopped, 19 / 21 judged wrong frames and 198 / 193 ms
in all (fails at 150); A2 → bass E 24 / 26; an A2 ringing over a bass E showed its common period,
27.5 Hz, in 65–270 frames; no-gap octave leaps into a weak fundamental 18–26 wrong frames.

## Decision

- **An `ambiguous octave` frame moves the needle** (owner, 2026-10-23): it shows the letter and
  the cents, which an octave error keeps, without the octave number. The time until the octave is
  known is measured and bounded separately.
- **An estimate whose range cannot test the octave below is provisional** (High below 213.8 Hz,
  Mid from 56.6 to 100.4 Hz) until a **verifier**, a lower range that sees `f/2` analysed
  alongside (Mid and Low under High, Low under Mid), reads it in a window started after the note.
  The verifiers generalise `T-003b`'s Low reference, keeping multiples 2–4, freshness, two agreeing
  frames and the 250 ms stuck-state bound. A verifier's reading passes the same tests as an
  estimate before it verifies anything. `Selection::Fixed` runs none, so its octaves stay ambiguous.
- **Nothing below 28.32 Hz is a note** (`LOWEST_FUNDAMENTAL_HZ`): it is a chord's common period.
- **The estimate guard (§3 (a), (b)) is one mechanism.** A jump of more than 600 ¢ from a locked
  estimate waits for three agreeing frames, giving `None`; a comparable (0.9) peak at τ/2 or 2τ that
  is the held note is shown instead, ambiguous. The third frame always wins: the octave below any
  note is as clear as the note (≥ 0.999), so an unbounded preference would hold E2 → E3 on E2.
- **A note an octave above a string still ringing is decided:** ambiguous while both sound, never
  moved down onto the lower string, whether a verifier or the selected range reads it. The guard
  remembers the note it left by moving up an octave and shows it ambiguous until it has been read
  for a window without the upper one. Before any move, the octave above entering a held note (its
  τ/2 NSDF level rising by `RISE` = 0.5) makes the held note ambiguous.
- **After a locked note, which may still sound, estimates must prove they are one note.** An octave
  of it, or a barely clear one (< 0.75), must repeat at 2τ (0.975, `REPEATS`); another note its
  range cannot test needs clarity 0.85 (`AFTER_NOTE_CLARITY`) unless a lower range reads it too.
  Two notes sounding together read as neither, near one of them. **Single notes win over chord
  rejection** (owner, 2026-10-25): a bar on every new note slowed notes after a gap and dropped
  noisy ones, so detuned chords and A2 over a low B are `T-014`'s, recorded and not gated. The
  locked note is forgotten once no range has read it for Low's window.
- **A gap shorter than the gate's hold is not scanned through.** Frames with the gate open only by
  its hold (`GateState::Holding`) are no misses, so the chain stays where the last note put it.
  Counting them scanned away from the next note's range by chance: A3 → A2 after 0.15 s, up to
  155 ms (fails at 150). Treating the hold as silence was rejected: it would reset the guard
  between two plucks of one string.
- **An estimate its range cannot test needs clarity 0.75** (`UNVERIFIED_CLARITY`), above `Mpm`'s 0.6:
  a partial read by a range blind to its fundamental is skewed by the other partials (±32 ¢ at 0.6).

**This supersedes in part `adr/0028`**, whose body stands otherwise:
- its *Low reference frame* bullet (Decision): replaced by the verifiers above;
- its *"The ledger and its tests measure from silence … T-003c-octave-guard owes it"*
  (Consequences): downward changes are gated, `AUDIO_ENGINE.md` §1, and two in `T-003f`;
- its CPU figure, "about 0.6 %": now 0.78 % of a host core, below.

## Consequences

- **Measured** (`AUDIO_ENGINE.md` §1's table; `tests/chain.rs`): no judged wrong frame outside
  chords in the leap sweep (sine, sawtooth, weak, pinch), the downward changes, the detuned ones
  (±30 ¢ and the review's offsets), the octave above a ringing string (E2/E3 … G3/G4, 10–12 dB),
  pinch harmonics and a weak low B from every home. E4 → low E, stopped: 118.3 / 117.1 ms in all
  (110 / 150); A2 → bass E 178.3 / 181.9 ms (190 / 260). The gated chord (A2 over a bass E) shows
  no third note.
- **The margins were checked on held-out fixtures** (D3/bass A, G2/low B, C3/bass E, E3/low E at
  ±7, ±22, ±45 ¢; twelve octave-above pairs at 10–12 dB; after-gap sweeps over 8 phases): every
  gated case passes. Their *loud chords* do not; see the gap below.
- **Known gap, `T-014-consonant-chords`:** two strings ringing loud together are handled only for
  the families gated here. A consonant interval is periodic at the common period (A2 + D3 reads D1,
  D3 + G3 G1, B3 + E4 E2, at clarity 1.0), and no periodicity test can tell it from one note. Other
  loud chords read the old note's sub-harmonic (G2 over low B, C3 over bass E; A2 +60 to +100 ¢ over
  bass E) at clarity up to 0.97. `PRODUCT_SPEC.md` §2's "hold the last confident estimate" needs
  onset detection, not periodicity.
- **Two downward changes' 260 ms gate is `T-003f`'s** (owner, 2026-10-23): A2 ringing 12 dB under
  a bass E (τ 3 s), 393 / 466 ms, or a low B (τ 1.5 s), 648 / 421 ms. The right note is shown in
  time, but Low's estimate wanders past ±5 ¢ in single frames; held to ceilings (+ 20 ms) until then.
- **Octaves are known later than the needle:** a low E ~170 ms after a downward change, ~140 ms
  from silence (only Low sees 41.2 Hz); a G3 ~65 ms; bounded by the verifier's span plus two hops.
- **Onsets after a short gap wait for their own scan:** A2 → bass E after 0.1 s, 85 / 88.4 ms (was
  50 / 53.5, when a scan through the gap sometimes reached Low first). E4 ringing loud over low E,
  recorded only: 115 / 114.6 ms (was 12.5 / 11.3).
- **CPU:** in High, a Mid and a Low frame per hop, and the guard; 0.78 % of a host core at worst
  (`T-003c`'s task file has the bench figures). The phone figure is still owed.

## Options rejected

- **Low as the only verifier:** E4 → low E took 77.5 / 76.5 ms instead of 62.5 / 61.3.
- **Showing the barely clear blind partial:** cents off by up to 32 ¢, and it restarted the
  verifiers' freshness (A2 → bass E 160 ms instead of 115).
- **The repeat test on every estimate:** onsets from silence slowed (110 Hz, 17.5 → 40 ms), and a
  melody over a bass, which repeats only 0.72–0.82, was dropped.
- **A clarity bar alone, or a repeat test past N/2 for Low below 40 Hz:** measured, the long-lag
  level passed correct low B frames only 75–85 % of the time and chord frames up to 27 %.
- **A peak search for the τ/2 level:** it flickers where a peak is barely formed, and a weak
  fundamental's steady 0.6 then read as the octave above entering.
- **Raising `Mpm`'s clarity floor for every estimate:** it changes onsets and the gate's timing for
  a problem only notes after a note, and blind ranges, have.
