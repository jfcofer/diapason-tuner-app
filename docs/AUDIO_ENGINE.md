# Audio engine

This document owns the **algorithms and real-time contracts** inside `rust/crates/{dsp,engine}`:
how pitch is detected, how beats are scheduled, and the numbers any implementation must hit.
Platform I/O configuration is in `PLATFORM_AUDIO.md`.

## 1. Performance budgets

These are acceptance criteria, not aspirations. `just bench` asserts the DSP ones; the end-to-end
ones are measured on the reference devices in `TESTING.md` §7.

| Budget | Target | Fails at |
|---|---|---|
| Perceived tuner latency (string plucked → needle moves), f₀ ≥ 160 Hz | ≤ 85 ms | > 120 ms |
| Same, 80–160 Hz | ≤ 110 ms | > 150 ms |
| Same, bass low B (≈ 31 Hz) | ≤ 190 ms | > 260 ms |
| Pitch accuracy, synthetic steady tone 30–1400 Hz | ±0.5 cent | ±1 cent |
| Pitch accuracy, recorded acoustic guitar fixtures | ±2 cents | ±4 cents |
| Display stability, held note | ≤ 1 cent peak-to-peak wobble | visible flutter |
| Metronome inter-onset jitter | 0 samples (exact) | any drift over 30 min |
| Analysis frame cost (4096-pt NSDF) | ≤ 1.5 ms on a 2021 mid-range phone | > 4 ms |
| CPU while tuning | ≤ 8 % of one core | > 15 % |
| Audio callback worst case | ≤ 15 % of one burst's period (`adr/0025`) | > 40 % |
| Added binary size per ABI | ≤ 2.5 MB | > 4 MB |

**Latency ledger.** Plucked string to moving needle, worst case, stage by stage, in ms at 48 kHz
(44.1 kHz is within 0.4 ms). The constants live in `diapason_dsp::range` (`adr/0028`). Two tests
bind this table:
- `range::tests`, `the_ledger_table_in_audio_engine_section_1_matches_the_code`, reads this file,
  parses every row of the table and the budget rows above, and compares them with the code, so
  a change on either side fails until the other matches;
- `tests/chain.rs`, `onset_latency_is_within_the_ledger`, measures each range's chain line at both
  rates: silence, then a tone, to the first estimate within ±5 ¢ that stays, on the audio clock.

| Stage | Low (from 30 Hz) | Mid (from 75 Hz) | High (from 160 Hz) |
|---|---|---|---|
| Input I/O, provisional until `T-002f` (one 960-frame burst) | 20 | 20 | 20 |
| Filter group delay (129 or 29 samples) | 2.7 | 2.7 | 0.6 |
| Window fill (three periods of the floor) | 100 | 40 | 18.75 |
| Settling (the DC block's transient; High needs none, measured) | 15 | 5 | 0 |
| One hop, waiting for the next frame | 5 | 2.5 | 2.5 |
| **Chain line, asserted** (measured worst, both rates) | **122.7** (117.1) | **50.2** (49.1) | **21.9** (21.7) |
| Smoothing reserve: two hops for `T-003f`'s median of five | 10 | 5 | 5 |
| Snapshot period: tuner snapshots at 60 Hz | 16.7 | 16.7 | 16.7 |
| One display frame (60 Hz) | 16.7 | 16.7 | 16.7 |
| **Total** (budget) | **186.0** (190) | **108.5** (110) | **80.2** (85) |

The latency budgets were moved on 2026-10-23, by the owner's decision (`adr/0028`). The first
ledger, with 30 Hz snapshots, showed I/O, snapshot and display alone summing to 70 ms, more than
the old 60 ms target before any analysis. Tuner snapshots now publish at the display rate, and
each target sits just above its range's worst case. Under low-power mode the tuner trades latency
for battery, and the snapshot line of this ledger does not apply.

**The ledger measures from silence; a note that follows another is gated on its own**
(`tests/chain.rs`, `a_downward_change_meets_the_fail_lines_and_never_shows_a_wrong_octave`,
`adr/0029`). Moving *down* a range is the costly case: the range playing cannot see the new note's
fundamental and reads a partial, until a range below, analysed alongside, moves the chain (§2). Two
times are measured from the new note's onset, each to the first frame from which every frame holds:
- **the needle moves:** the right pitch class within ±5 ¢. A frame flagged `ambiguous octave`
  counts (owner, 2026-10-23), because an octave error keeps the letter and the cents;
- **the octave is known:** the right note within ±5 ¢, not flagged.

A total adds the target range's other ledger lines (58.3 ms for Mid, 63.3 ms for Low) to the
needle's time. The new notes have a weak fundamental (`synth::WEAK_FUNDAMENTAL`, `WEAK_LOW_B`). In
every case below, no frame shows a wrong note once judged by the leap rule (§2).

| Downward change, 48 / 44.1 kHz | Needle (ms) | Total (ms) | Octave known (ms) | Target / fails at |
|---|---|---|---|---|
| E4 → low E, E4 stopped | 60 / 58.7 | 118.3 / 117.1 | 170 / 170.5 | 110 / 150 |
| E4 → low E, after a 0.1 s gap | 37.5 / 37.5 | 95.8 / 95.9 | 145 / 149.2 | 110 / 150 |
| E4 → low E, E4 rings 12 dB under (τ 0.6–3 s) | 55 / 53.7 | 113.3 / 112.1 | 165 / 165.4 | 110 / 150 |
| A2 → bass E, A2 stopped | 115 / 118.6 | 178.3 / 181.9 | as the needle | 190 / 260 |
| A2 → bass E, after a 0.1 s gap | 85 / 88.4 | 148.3 / 151.7 | as the needle | 190 / 260 |
| A2 → bass E, A2 rings 12 dB under, τ 0.6 / 1.5 s | 110, 185 / 148.5 | 173.3, 248.3 / 211.8 | as the needle | 190 / 260 |
| A2 → low B, A2 stopped | 115 / 113.6 | 178.3 / 176.9 | as the needle | 190 / 260 |
| **Gated in `T-003f`:** A2 → bass E, 12 dB under, τ 3 s | 330 / 402.9 | 393.3 / 466.3 | as the needle | 190 / 260 |
| **Gated in `T-003f`:** A2 → low B, 12 dB under, τ 1.5 s | 585 / 358.0 | 648.3 / 421.4 | as the needle | 190 / 260 |

Every change gated here meets its fail line. The last two miss it on cents, not octave, and their
260 ms gate is `T-003f`'s (owner, 2026-10-23). The right note, octave known, is shown from
99–104 ms (totals 162–167 ms). Low's estimate under a string ringing 12 dB down then wanders past
±5 ¢ in single frames, by up to 6.1 ¢ under a bass E and 8 ¢ under a low B, for as long as that
string lasts: a three-period window's limit. `T-003f`'s median of five, which the smoothing line
reserves, removes single frames. Until then the test holds both to regression ceilings
(`Gate::Shortfall`: these totals plus 20 ms). A low E's octave is known only once Low, which can
see 41.2 Hz, reads it (§2), so 105–110 ms after the needle.

**A gap shorter than the gate's 250 ms hold** is no silence to the selector: the gate stays open,
and its frames find no pitch. They are no misses (`Observation::Holding`, §2), so the chain stays
where the last note put it, as if that note had stopped. Counted as misses, they scanned it away
from the next note's range by chance of where the onset fell: an A2 after an A3 and 0.15 s took up
to 155 ms in all (fails at 150). Gated over 0.15 and 0.2 s gaps, each at four onset offsets, sine
and weak fundamental, both rates (`tests/chain.rs`,
`a_downward_change_after_a_gap_the_hold_spans_meets_the_fail_line`): A3 → A2 totals 78–101 ms,
A1 → low B 129–180 ms. The cost: a note whose range a lucky scan had reached during the gap now
waits for its own scan, so A2 → bass E after 0.1 s takes 148.3 / 151.7 ms in all (was 113.3 /
116.8), inside its target.

Two louder cases are recorded only. E4 ringing at the new note's level is a harmonic of it: the
needle moves in 115 / 114.6 ms, and the octave is known in 225 / 223.8 ms. A2 ringing over a bass
E is a chord. It shows `None`, ambiguity or the old note, never a third note: not the notes' common
period, and not a reading near the old one that is no single note (§3). **That holds for that chord
only:** since single notes win over chord rejection (owner, 2026-10-25), A2 ringing over a low B and
the detuned chords (`tests/chain.rs`,
`a_detuned_downward_change_or_chord_never_shows_a_wrong_or_third_note`) are recorded, not gated,
and loud chords in general are not handled: a consonant interval (a fourth or fifth: A2 + D3 reads D1, D3 + G3 G1, B3 + E4 E2, at
clarity 1.0) is periodic at the common period and cannot be told from one note by periodicity, and
other pairs read the old note's sub-harmonic (G2 over low B, C3 over bass E). That is
`T-014-consonant-chords`, a known gap that predates `T-003c`.

## 2. Signal chain (tuner)

```
          ┌──────── chain::Front, audio callback ────────┐
input ──▶ │ DC block ──▶ gate                              │ ──▶ ring buffer: samples + gate state
          └───────────────▲──────────────────────────────┘                    │
                          └── "unpitched" (one flag back) ◀──┐                 ▼
          ┌──────── chain::Analysis, analysis thread ─────────┴───────────────────────────┐
          │ Low, Mid: decimate ×4 (anti-aliased)   High: band-limit at full rate          │
          │      ▼                                                                         │
          │ frames per range ──▶ NSDF via FFT ──▶ peak pick ──▶ parabolic interpolation    │
          │                                                     (common overlap, §3)       │
          │ octave guard (§3) ──▶ range selector (hysteresis, scan; verifier frames below) │
          └──────▼─────────────────────────────────────────────────────────────────────────┘
   estimate + octave flag ──▶ median(5) ──▶ one-euro filter ──▶ snapshot (60 Hz)
```

**Pre-processing.** A one-pole high-pass at 20 Hz removes DC and handling rumble
(`dsp::filter::DcBlock`). An RMS gate (default −50 dBFS, adaptive to the noise floor measured over
the last 2 s) decides when nothing is being played — a tuner that hunts in silence feels broken.
Both run in the callback, as `dsp::chain::Front`. The gate's state crosses the ring buffer with
each block. The gate never mutes the audio the detector sees; it decides only whether an estimate
is shown (`dsp::gate::NoiseGate`).

Its floor learns while the gate is closed. It must not learn while a note is held, or the note
would become the floor and close the gate. So it learns while open only when the analysis side
reports the open gate *unpitched*: no clear estimate for 2 s (`dsp::chain::UNPITCHED_SECONDS`).
That is how a room louder than −50 dBFS raises the gate (`PRODUCT_SPEC.md` §2). The engine
carries that one flag back from the analysis thread.

**Band-limiting and decimation.** Parabolic interpolation is biased by harmonics near the
*analysis* Nyquist (`T-003a`), so both paths keep their content below an eighth of their analysis
rate, through linear-phase Kaiser FIRs designed per rate (`dsp::range::RangeTable`). The Low and
Mid ranges analyse at a quarter of the input rate (12 kHz from 48 kHz, 11.025 kHz from 44.1 kHz),
which cuts their NSDF cost about 4×. High analyses at the full rate. The crossover is the Mid/High
boundary, 160 Hz, and it is a latency choice. In Mid's window the decimated path keeps a 2×
margin to at least 320 Hz (`T-003b`). Notes above 160 Hz still go to High, because its window is
18.75 ms, not Mid's 40 ms. High runs at full rate because the decimator's content stops at
1.5 kHz, and High goes to 1400 Hz. `dsp::chain::Analysis` runs both paths on every sample.

Range changes are hysteretic, to avoid oscillating at a boundary (`dsp::range::RangeSelector`).
The overlaps sit where they cannot cross a latency row of §1:
- Low holds up to 79.46 Hz, a semitone past Mid's floor and short of the 80 Hz row;
- Mid holds from 70.79 Hz up to 160 Hz exclusive;
- High holds down to 151 Hz.

So every f₀ ≥ 160 Hz is High, and every f₀ ≥ 80 Hz is out of Low. A pitch above the band moves
at once. A pitch below it moves only after it has held for a whole window of the current range.
A window spanning a note change finds the two notes' common period, which is lower than either,
and would otherwise drop the chain into a range showing it.

**Window sizing is adaptive**, because a fixed window either misses low strings or is needlessly
slow for high ones. A window holds three periods of its range's floor, and the hop is short,
because each hop is paid three times in latency (§1, `adr/0028`):

| Range | Floor | Analysis rate | Window @ 48 / 44.1 kHz | Hop | Update rate |
|---|---|---|---|---|---|
| Low (5-string bass B to its D string, 73.4 Hz) | 30 Hz | ÷ 4 | 1200 / 1103 (100 ms) | 5 ms | 200 Hz |
| Mid (guitar low E to D3, the bass's G) | 75 Hz | ÷ 4 | 480 / 441 (40 ms) | 2.5 ms | 400 Hz |
| High (guitar G3 and up, violin, uke) | 160 Hz | full | 900 / 827 (18.75 ms) | 2.5 ms | 400 Hz |

*Note.* The windows are short because a window that is only partly filled with a new note biases
the NSDF, so the first good estimate waits for the window to fill (§1's ledger). The selector
returns to the home range on silence. The instrument preset picks that range (M3), so the common
case (guitar E-standard, Mid) never pays the bass window. A note the home range cannot see (1.4 kHz
from Low is in the decimator's stopband) is found by scanning after a window and 50 ms of unclear
frames. The scan runs downward (High → Mid → Low → High), since a range that hears nothing is most
often above the note. Every range's history is kept full, so a switch costs one hop. Only frames
with the gate open by its level count: while it only holds over a gap (`GateState::Holding`)
nothing new is taken to be played, so there is nothing to scan for (§1, `T-003c`). A note started
under the close threshold during the hold waits for the hold to end before a scan finds it.

**Verifiers decide the octave a range cannot test** (`adr/0029`). A range cannot see below 1.5
periods of its floor, since `Mpm` searches lags up to half the window (`RangeTable::sees`): High is
blind below 106.9 Hz, Mid below 50.2 Hz (107.0 and 50.3 at 44.1 kHz). So a range cannot test the
octave below an estimate under twice that: High's below 213.8 Hz, Mid's from 56.6 to 100.4 Hz
(`RangeTable::tests_octave`; under 56.6 Hz the octave below is under `LOWEST_FUNDAMENTAL_HZ`,
28.32 Hz, and no note). A low E with a weak fundamental, heard by High, reads as its second
partial, which High holds. So each frame of a range below the selected one is analysed too, as a
**verifier**: Mid and Low under High, Low under Mid. An estimate whose range cannot test its octave
is flagged `ambiguous octave` until a verifier that sees the octave below reads the estimate itself
in a window that started after the note did (`RangeSelector::octave`). A verifier's pitch below the
current band moves the chain to that pitch's range under four conditions:
- the current range's last clear pitch is 2, 3 or 4 times it, or there is none;
- the verifier window started after the last switch, pitch change or end of a held reading, since a
  window spanning a note change reads the notes' common period;
- two such frames of that verifier agree, or such frames have agreed for 250 ms (a bound on how
  long a wrong reading can hold a range, `REFERENCE_PATIENCE_SECONDS`);
- it is at least 28.32 Hz, and it is not the octave below a pitch the chain has just moved up to.
  That would be the lower string still ringing under the one just plucked, which stays ambiguous;
  the selected range reading it does not move the chain down either (`RangeSelector::observe`).

A verifier's reading must pass the same tests as an estimate (§3, after a note) before it verifies
an octave, or a chord's reading would. `Selection::Fixed` runs no verifier: a range locked by hand
leaves the octaves it cannot test ambiguous.

Two more rules keep guesses off the display, and §3's rules after a note more. No estimate below 28.32 Hz is shown: it is a chord's
common period (A2 over a bass E reads 27.5 Hz). An estimate whose octave its range cannot test is
taken only from clarity 0.75 (`octave::UNVERIFIED_CLARITY`). Such a reading is a partial of a
fundamental the range cannot see, and the other partials skew it: Mid's reading of a weak bass E at
clarity 0.6 is up to 32 ¢ off.

From silence, a low E is in Mid within 25 ms and its octave is known about 140 ms after the onset;
a G3 is known about 65 ms after (`tests/chain.rs`,
`an_octave_the_range_cannot_test_is_ambiguous_until_a_lower_range_reads_the_note`, which bounds
it by the verifier's span and two of its hops). In High this costs a Mid and a Low frame per hop
(0.78 % of a host core in all, `chain/analysis`). A no-gap leap sweep (sine, sawtooth, weak fundamental, pinch
harmonic) asserts that no judged frame shows a wrong note and that each leap switches at most
once.

## 3. Pitch detection: NSDF / McLeod (MPM)

`adr/0005-pitch-detection-algorithm.md` records why, in short: autocorrelation alone is
octave-error-prone; plain YIN is good but its cumulative-mean normalisation is less robust on the
strong-harmonic, inharmonic content of a plucked steel string; **MPM's normalised square difference
function gives a clarity measure that doubles as a confidence signal**, which the UI needs anyway
for the lock indicator. FFT-accelerated so cost is O(N log N), not O(N²).

Implementation contract:

1. Compute `r(τ)` (autocorrelation) via FFT: forward on zero-padded window, magnitude squared,
   inverse. Reuse preallocated plans and scratch buffers — one `FftPlanner` built at stream start.
2. Compute `m(τ)` (the cumulative squared-magnitude term) incrementally in O(N).
3. `n(τ) = 2r(τ) / m(τ)` — the NSDF, bounded in [−1, 1].
4. Pick key maxima: the highest maximum between each pair of positive zero crossings. Take the
   first key maximum exceeding `k · max(n)` with `k = 0.9` (configurable; lower = more sensitive,
   more octave errors).
5. Parabolic interpolation over the three points around the chosen maximum → sub-sample period →
   frequency. This is what buys sub-cent resolution from a coarse lag grid. The three points are
   `n̄`, recomputed over one common overlap counted from both ends of the frame
   (`pitch::Mpm::common_overlap`; the decision is `adr/0028`). The FFT NSDF `n` sums a different
   number of pairs at each lag, so its three samples come from three different sums. A
   sawtooth's reset at the window's edge tilts them, most when the window holds whole periods.
   On `tests/chain.rs`'s headline grid (24 points per octave plus every whole-period frequency
   ±0.3 Hz, 8 phases) the worst is −0.507 ¢ without the common overlap (75 Hz, 44.1 kHz, Mid)
   and 0.087 ¢ with it.
6. **Clarity** = the value of the parabola through `n̄` at its vertex (step 5); step 4 picks
   peaks on the FFT `n`. A vertex more than one lag from the chosen maximum is no peak, and the
   frame gives no estimate. Below 0.6 the estimate is discarded and the UI shows "listening"
   rather than a wrong note.

**Octave guard.** The most visible failure mode of any tuner (`dsp::octave`, `adr/0029`). Two
defences, which work as one, over the selected range's frames:
- **(b) Hysteresis.** A jump of more than 600 ¢ (`JUMP_CENTS`) from a *locked* estimate (accepted,
  within 50 ¢ of itself, for a window of frames) needs three consecutive frames agreeing within
  50 ¢ (`CONFIRM_FRAMES`). Until then the frame gives `None`, never the old note. From an estimate
  not yet locked, a jump is taken at once, so an onset settles at its own pace.
- **(a) The τ/2 and 2τ test.** While such a jump waits, `Mpm::peak_near` reads the NSDF's peaks at
  half and twice the chosen lag, refined like the period (step 5, O(1)). If one has *comparable*
  clarity, at least 0.9 of the chosen peak's (`COMPARABLE`, mirroring `k`), and it is the held note,
  the frame shows it, flagged `ambiguous octave`. The held note is the one consistent with the
  previous estimate; the frame alone cannot tell the octaves apart. While the pick keeps flipping
  between octaves, the held note stays flagged for a window.
- **Why a third frame wins even against a comparable peak.** Every periodic signal is also periodic
  at 2τ, so the octave below a note is as clear as the note: measured, at least 0.999 of it. A weak
  fundamental's second partial reads 0.55–0.65 of it (`pitch::tests`). Preferring the old octave
  without a bound would hold a leap from E2 to E3 on E2 for good.
- **After a note, estimates must prove they are one note.** While a note shown with its octave
  known for a window may still sound, two strings sound together and read as neither, near one of
  them. An estimate that is barely clear (under 0.75) or an octave of that note must repeat at 2τ:
  its peak there at least 0.975 of its clarity (`REPEATS`); where the range cannot search 2τ (Mid at
  55 Hz), it stays `ambiguous octave` until a verifier reads it. An estimate of another note that
  its range cannot test needs clarity 0.85 (`AFTER_NOTE_CLARITY`), unless a lower range reads it
  too; an estimate its range can test needs nothing more. A frame failing a test is withheld, no
  miss. The locked note is forgotten once no range has read it for Low's window
  (`OctaveGuard::heard`), or when the gate closes. Measured on filled windows: correct readings
  repeat at ≥ 0.9895 and are ≥ 0.9 clear, the third notes of A2 over a bass E or a low B and of E4
  over low E (each ±30 ¢) repeat at ≤ 0.959 and are ≤ 0.833 clear. A melody over a bass (repeats
  0.72–0.82, clarity ≥ 0.775) is the held note. **Single notes win over chord rejection** (owner,
  2026-10-25): a bar on every new note after a locked one slowed notes after a gap and dropped
  noisy ones, so only blind estimates pay it (`T-003c`, review round 4).
- **The octave above a held note.** A string plucked an octave above one still ringing, in a range
  that sees both, reads as the lower one. Its τ/2 NSDF level (`Mpm::value_at`, a level and not a
  peak, which comes and goes where barely formed) rising by `RISE` (0.5) over its floor since the
  lock makes the held note ambiguous. And the note the estimate left by moving up an octave stays
  ambiguous until it has been read for a window without the upper one in between (G3 under G4,
  read by a range that sees both).
- **Not handled:** loud chords other than those families, consonant intervals above all, which
  are periodic at their common period (`T-014-consonant-chords`, §1).

Tests: `octave::tests` covers both cases of (b), the hysteresis holding and giving way, the
threshold at its edge and in absolute terms, and which estimates must repeat. `tests/chain.rs` holds the leap sweep to zero wrong
frames. The octave of notes a range cannot test is the range selector's (§2).

**Smoothing.** Two separate values leave the engine, and conflating them is a design error:
- `frequency_raw` — unsmoothed, used for lock detection and the strobe phase.
- `frequency_display` — median-of-5 then a one-euro filter (`min_cutoff = 0.6`, `beta = 0.02`).
  One-euro rather than a fixed low-pass because it gives a still needle at rest *and* a fast needle
  when the player turns the peg, which a single time constant cannot do.

**Lock detection.** `|cents| ≤ tolerance` (default ±3, configurable 1–10) for ≥ 250 ms of
consecutive frames with clarity > 0.8. Unlock has a wider band (tolerance + 1.5 cents) so the
indicator does not chatter at the boundary.

## 4. Note and temperament math

`cents = 1200 · log₂(f / f_target)`. Target frequency depends on four user settings that compose:

- **Reference A4** — 415–466 Hz, default 440.0, 0.1 Hz steps.
- **Temperament** — Equal (default), Just (relative to a chosen key), Pythagorean, Werckmeister III,
  Kirnberger III, Vallotti, and a "sweetened guitar" table. Each is a 12-entry cents-offset table
  applied to the equal-tempered target.
- **Transposition** — ±12 semitones, for capo and transposing instruments.
- **Per-string offsets** — a stored table for players who compensate specific strings.

The table lives in `fixtures/note_table.json` and is the shared truth for the Rust and Dart
implementations (`ARCHITECTURE.md` §3).

## 5. Metronome scheduling (drift-free by construction)

**Never compute the next beat from the previous beat's timestamp — accumulate in integer samples.**

```
samples_per_beat : Q32.32 fixed point = (sample_rate · 60 · 2³²) / bpm
phase            : Q32.32, advanced by frames_in_this_callback each callback
```

Each callback: consume commands, then walk `phase` forward. Every time the integer part crosses
`samples_per_beat`, a beat onset falls at a known **sub-buffer sample offset** — the click is
rendered starting at that exact offset inside the buffer, not at the buffer boundary. This is the
difference between a metronome that is accurate to the buffer (±5–10 ms, audibly loose) and one
that is sample-exact. Fixed-point rather than float because f64 accumulation over an hour at
480 samples/callback accumulates measurable error; a Q32.32 accumulator does not.

Tempo changes take effect at the next beat boundary by default (musically correct) or immediately
in "trainer" mode; both are the same code path with a flag, and both preserve phase.

**Click synthesis is procedural — no audio assets.** A click is a short exponentially-decaying
sine/triangle blend plus an optional filtered-noise transient: `accent` ≈ 1600 Hz, `beat` ≈ 1000 Hz,
`subdivision` ≈ 800 Hz at lower gain, 25–40 ms decay. Keeps the binary small, sidesteps sample
licensing entirely, allows continuous timbre parameters, and is trivially deterministic in tests.
Sample-based voice packs are a post-1.0 feature and would arrive as a `ClickSource` trait impl.

Voices are polyphonic (a subdivision may still be ringing when the next beat starts) with a fixed
pool of 8 preallocated voices. Voice steal is oldest-first.

## 6. Visual/haptic synchronisation

The click is produced *ahead of* being heard, by the output latency of the device (5–40 ms). If the
UI flashes when the engine renders, it flashes early; if it flashes when Dart receives a message,
it flashes late and jittery. Correct approach:

1. On each beat the engine records `(beat_index, output_frame_index)`.
2. `audio_io` converts `output_frame_index` to a **host clock timestamp** using the platform's
   stream timestamp API (`AAudioStream_getTimestamp` / `AudioTimeStamp.mHostTime`), which already
   accounts for the device's output latency.
3. The snapshot carries `next_beat_at_monotonic_ns`.
4. Flutter runs a `Ticker` and interpolates toward that timestamp, so the animation *arrives* on the
   beat regardless of message-delivery jitter. Frames are cheap; correctness is in the target time.
5. Haptics fire from the same timestamp, scheduled slightly early to compensate for actuator
   latency (~10 ms, calibratable). Haptics **never** define the beat.

## 7. Real-time safety, enforced

The rules are in `AGENTS.md` §6; this is how they are enforced rather than merely intended:

- The callback body runs inside `assert_no_alloc`, which is compiled out of release builds (its
  `disable_release` feature). It traps wherever its allocator is the global one. Today that is the
  `engine/tests/no_alloc.rs` integration test: it drives 10 s of audio through the offline backend
  and fails on a single allocation, and a canary proves the trap fires. On Android the AAudio
  trampoline also runs every callback inside it, and `just test-android-device` proves that on
  hardware with its own canary (`adr/0020`). Debug builds of the app install it as
  `diapason_ffi`'s global allocator, so `just test-integration-android` runs the real engine with
  the trap armed. It sees only Rust's allocator, not allocations inside the platform's audio
  libraries.
- Clippy lints denied on the RT path: `disallowed-methods` (`Vec::push`, `HashMap::insert`,
  `Instant::now`, `Mutex::lock`, …) and `disallowed-macros` (`println!`, `format!`, …), listed
  once in `rust/crates/engine/clippy.toml`; `dsp` links to the same file. `audio_io` does not:
  its callback path is small and held to the rules at run time instead (`adr/0020`).
- The platform calls the backend must make on the RT path are audited, not banned:
  - AAudio's non-blocking `AAudioStream_read` and its `getTimestamp`. On the legacy (non-MMAP)
    path each takes a short platform mutex, the cost Oboe's `FullDuplexStream` also accepts;
  - `clock_gettime(CLOCK_MONOTONIC)` (`adr/0020`).
- Command queue: `rtrb` SPSC. Snapshot publishing: `triple_buffer`. No other cross-thread primitive
  is permitted in `engine`.
- All buffers are allocated in `Engine::prepare(max_block_size, sample_rate)` — the same lifecycle
  hook every audio plugin API has, and for the same reason.
- Logging from the RT thread goes into a preallocated lock-free ring of fixed-size records that a
  normal-priority thread drains. In release builds it is compiled out.

## 8. Testing hooks the design provides

- `Offline` backend feeds a WAV or generated buffer through the identical engine, deterministically
  and faster than real time — every accuracy and scheduling test uses it.
- Fixtures: synthesised sines, sawtooths and an inharmonic plucked-string model, generated in code
  at test time by `diapason_dsp::synth` (`adr/0027`). Recorded samples of guitar, bass, ukulele and
  violin at known pitches, with vibrato, with room noise, and detuned by known cent offsets, are
  the only fixtures checked in (`fixtures/audio/`, `T-003e`).
- Scheduling test: run 30 minutes of audio through the offline backend at 47 different tempos and
  assert every onset lands on the exact expected sample index. Zero tolerance.
