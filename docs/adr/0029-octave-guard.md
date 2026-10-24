# 0029 — Octave guard: provisional octaves, verified by the ranges below

**Status:** Accepted · 2026-10-23 (the ambiguity rule decided by the owner, 2026-10-23)

## Context

`T-003b` (`adr/0028`) left octave correctness across ranges open. `Mpm` searches lags up to half its
window, so a range cannot see below 1.5 periods of its floor. High is blind below 106.9 Hz, Mid
below 50.2 Hz. A low note with a weak fundamental, heard by a range above it, reads as its second
partial. Each fix tried inside the range selector traded one octave error for another.

Measured before this decision (`T-003c`'s task file, home Mid, new notes weak, 48 / 44.1 kHz):
- **Downward changes showed wrong notes.** E4 → low E, stopped: 19 / 21 judged wrong frames, and
  the right octave after 140 / 135 ms (198 / 193 ms in all, against 150). A2 → bass E: 24 / 26,
  and 160 / 159 ms.
- **Chords showed a third note.** An A2 ringing over a bass E showed Low's common period, 27.5 Hz
  (A0), in 65–270 frames.
- **No-gap octave leaps down** into a weak fundamental showed 18–26 wrong frames.

## Decision

- **An `ambiguous octave` frame moves the needle** (owner, 2026-10-23). An octave error keeps the
  pitch class and the cents, so such a frame shows the letter and the cents without the octave
  number. It counts for `AUDIO_ENGINE.md` §1's latency budgets. The time until the octave is known
  is measured and bounded separately.
- **An estimate whose range cannot test the octave below is provisional.** Where `f/2` is beyond
  the range's longest lag (High below 213.8 Hz, Mid from 56.6 to 100.4 Hz), the estimate is flagged
  ambiguous. It stays so until a **verifier** reads the estimate itself in a window that started
  after the note did. A verifier is a lower range that sees `f/2`, analysed alongside: Mid and Low
  under High, Low under Mid.
- **The verifiers generalise `T-003b`'s Low reference** and keep all of its rules: multiples 2–4
  (`MAX_HARMONIC`), freshness, two agreeing frames, and the 250 ms stuck-state bound
  (`REFERENCE_PATIENCE_SECONDS`). A verifier reading the octave below moves the chain there. One
  reading neither the note nor a multiple below it moves nothing.
- **Nothing below 28.32 Hz is a note** (`LOWEST_FUNDAMENTAL_HZ`: Low's floor less a semitone, under
  the low B). It is neither shown nor acted on: it is a chord's common period.
- **The case that is ambiguous by nature is decided:** a note an octave above a string still
  ringing. Once the chain has moved up an octave, a verifier's reading of the octave below is the
  lower string. It moves nothing, and the note stays ambiguous while both sound. `T-003b`'s patience
  moved it down after 250 ms. When the lower string is the louder, the range that sees both reads
  it, and the tuner shows it.
- **An estimate whose octave its range cannot test needs clarity 0.75** (`UNVERIFIED_CLARITY`),
  above `Mpm`'s 0.6. A partial read by a range blind to its fundamental is skewed by the other
  partials: ±32 ¢ at clarity 0.60–0.64 for a weak bass E, and ±88 ¢ in a chord, a third pitch class.
- **The estimate-level guard (§3 (a), (b)) is one mechanism.** A jump of more than 600 ¢ from a
  locked estimate waits for three consecutive agreeing frames, and gives `None` meanwhile. If the
  held note has a comparable peak (0.9) at τ/2 or 2τ, that peak is shown instead, flagged
  ambiguous. The third agreeing frame always wins, because the octave below any note is as clear
  as the note (≥ 0.999): an unbounded preference would hold E2 → E3 on E2.
- **After a note, an estimate near it must be a period.** Once a note has been shown with its
  octave known for a window, an estimate that is barely clear (under 0.75), or an octave of that
  note, is taken only if its NSDF peak at 2τ reaches 0.975 of its clarity (`REPEATS`). Where the
  range cannot search 2τ, it stays ambiguous until a verifier reads it. A chord reads as neither
  note, but near one of them, and does not repeat. Measured: notes ≥ 0.9895, chords' third notes
  ≤ 0.959.

## Consequences

- **Measured after** (`AUDIO_ENGINE.md` §1 has the table):
  - judged wrong frames: none outside chords, in the leap sweep (sine, sawtooth, weak, pinch),
    the downward changes, pinch harmonics and a weak low B from every home;
  - E4 → low E, stopped: 120.8 / 119.7 ms in all (target 110, fails at 150);
  - A2 → bass E, stopped: 173.3 / 176.9 ms (190 / 260);
  - chords: no third note, known or ambiguous; `None`, ambiguity or the old note.
- **Two downward changes' 260 ms gate is `T-003f`'s** (owner, 2026-10-23). A bass E under an A2
  ringing 12 dB down with τ 3 s: 393 / 466 ms. A low B under it with τ 1.5 s: 648 / 421 ms. They
  miss on cents, not octave. The right note, octave known, is shown in time, but Low's three-period
  estimate under the interferer wanders past ±5 ¢ in single frames. `T-003f`'s median of five
  removes single frames; until then `tests/chain.rs` holds both to ceilings, these totals + 20 ms.
- **The octave of a low E is known ~170 ms after a downward change**, ~140 ms from silence. Only
  Low's 100 ms window sees 41.2 Hz. A G3's octave is known in ~65 ms. Both are bounded by a test:
  the verifier's span plus two hops.
- **The repeat test costs an onset after a gap** with the old note locked: A2 → bass E after
  0.1 s takes 60 / 58.5 ms instead of 50 / 53.5, because a half-silent window does not repeat.
- **CPU:** in High, a Mid and a Low frame per hop, and the guard (39 ns per frame). The worst case
  is 0.72 % of a host core (0.62 % before); the phone figure is still owed.
- **`chain::Frame` gains `octave`.** M3 shows the flag as `PRODUCT_SPEC.md` §2's `ambiguous octave`.

## Options rejected

- **Low as the only verifier** (`T-003b`): measured with that mutation, E4 → low E, stopped, takes
  77.5 / 76.5 ms instead of 62.5 / 61.3, reaching Mid only by scanning once High's misses run out.
  Every High note under 213.8 Hz would wait for Low's 100 ms window to know its octave. The
  mutation fails the ambiguity test.
- **Showing the barely clear partial as ambiguous** (clarity 0.6 to 0.75): its cents are off by up to
  32 ¢ and it flickers. Each such reading also restarted the verifiers' freshness. With it, A2 → bass
  E, stopped, took 160 / 158.5 ms; without it, 115 / 118.6. A2 → bass E, 12 dB under (τ 0.6 s,
  44.1 kHz) took 203 ms; without it, 148.5.
- **Preferring the previous octave without a bound** (§3 (a) alone): every octave leap up sticks.
- **Raising `Mpm`'s clarity floor for every estimate:** that changes §3 step 6, onsets and the gate's
  `unpitched` timing, for a problem only blind ranges have.
- **The repeat test on every estimate:** onsets from silence lost their early frames (110 Hz,
  17.5 → 40 ms), and a melody over a bass, which repeats only 0.72–0.82, was dropped. Scoping it to
  estimates *other than* the locked note leaked, because a chord's drifting reading of the old note
  re-locks it. A wider peak search (four lags) and filtering verifier readings changed nothing.
