# 0032 — The needle: a median of five, a one-euro filter in cents bounded to the median, and a lock from the caller's cents

**Status:** Accepted · 2026-10-30 (`T-003f`)

## Context

`AUDIO_ENGINE.md` §3 asks for two values: `frequency_raw`, unsmoothed, and `frequency_display`, a
median of five then a one-euro filter (`min_cutoff = 0.6`, `beta = 0.02`). It also asks for a lock
indicator. §1's ledger reserves two hops for the smoothing (`range::SMOOTHING_HOPS`), and §1's
stability row asks for at most 1 ¢ of wobble on a held note. The spec leaves open:

- what domain the filter runs in, since `beta` has a unit;
- the filter's derivative cutoff;
- what the smoothing does across a note change, a frame without an estimate, or an
  `ambiguous octave` frame. Mixed carelessly, it would undo the octave guard (`adr/0029`): a median
  over frames of two octaves shows neither;
- how the lock learns the target note, which depends on settings that `dsp` does not hold (§4).

Measured while building it (`tests/chain.rs`, over the onset-latency grid and held sawtooths with
seeded noise), the spec's filter at its rest cutoff misses the ledger. An onset does not step. The
estimate *converges* as the window fills, from as far as 45 ¢ off. A median of five follows that
ramp two frames behind, and the filter on top trails it further: the needle settled up to 18.8 ms
past the chain and smoothing lines (Low, a sine low B at 44.1 kHz: 147 ms against 133 ms). A
filter fast enough to meet the line (`beta` 0.3, `d_cutoff` 10 Hz) wobbles 6.3 ¢ on a held note
at 10 dB SNR. The spec's wobbles 0.70 ¢.

## Decision

- **The filter runs in cents** relative to the first value it showed, with `beta` in Hz per ¢/s
  and `d_cutoff = 1` Hz (the paper's default). In hertz, `beta` would act 45 times harder at
  1400 Hz than at 31 Hz. Its time step is the frames' positions on the audio clock.
- **The needle never trails the median by more than 3 ¢** (`smooth::MAX_LAG_CENTS`). After each
  step the filter's output is clamped to within 3 ¢ of the median, keeping its speed estimate. On a
  held note the output is already inside, so the filter alone decides. On a moving note the
  needle is within ±5 ¢ once the median is within 2 ¢, with no jump. Measured: the ledger holds
  for any bound up to 4 ¢ (4.5 ¢ misses by 3 ms), and the 10 dB-SNR wobble stays ≤ 1 ¢ for any
  bound from 3 ¢ (2 ¢: 2.8 ¢). The lower end keeps the latency margin.
- **The smoothing line is a worst case, as every line of the ledger is.** The needle's worst onset
  is within the chain line plus the smoothing line, and that is what is gated. Run by run the
  needle can lag its own estimate by more than two hops where the estimate settled early (up to
  30 ms in Low, measured in review), because the needle waits for the median to come within 2 ¢.
- **Smoothing never mixes notes, and never shows the old one.**
  - Frames whose pitch class is more than 50 ¢ (`octave::SAME_NOTE_CENTS`) from the held note's
    gather in a second median and show nothing. Three of them become the note, its first value a
    median: two hops, the ledger's line. A frame back on the held note first drops them, so one
    stray frame in an attack does not cost the held note a warm-up.
  - **The held note's pitch class in another octave is the same note.** The guard settling a held
    note's octave (`adr/0029`, known or `ambiguous`) moves the history by that octave, 2ᵏ, so the
    needle keeps its cents, neither blanks nor mixes octaves, and shows the frame's octave.
  - A frame without an estimate shows none. The history outlives five such frames, or frames of
    another note (`smooth::HELD_GAP_FRAMES`, the median's length, longer than a guard's
    confirmation); after a longer gap a note starts again. A closed gate resets. The octave flag is
    the frame's own.
- **The display lives in `chain::Analysis`** (`Frame::display`), so the engine still owns only
  threads (M3). `Frame::estimate` stays `frequency_raw`.
- **Lock is `dsp::lock::Lock`, fed with cents the caller computes** from the target note (§4: A4,
  temperament, transposition, per-string offsets). `dsp` holds no note table ahead of `T-003d`. It
  reads `frequency_raw`. It locks after ≥ 250 ms of positions within the tolerance with clarity
  > 0.8, and unlocks past tolerance + 1.5 ¢, or on a frame without an estimate or not clear. The
  tolerance is clamped to 1–10 ¢.

## Consequences

- **Measured, both rates:** the needle's worst onset is within each range's chain line plus its
  smoothing line, with 0.2 ms to spare at worst (High, 44.1 kHz). A chain change that keeps the
  estimate within its own line can still fail the needle's; that is the gate doing its job.
- Every downward change in §1 meets its fail line on the needle, including the two `T-003c` left
  to this slice: A2 → bass E 12 dB under, τ 3 s, 163.3 / 166.9 ms (was 393.3 / 466.3), and
  A2 → low B 12 dB under, τ 1.5 s, 163.3 / 191.9 ms (was 648.3 / 421.4). `Gate::Shortfall` is
  gone. The low B at 44.1 kHz is 1.9 ms past its 190 ms target, inside the 260 ms fail line.
- **Stability:** a held sawtooth 41.2–1318.5 Hz with noise at 20 dB SNR wobbles ≤ 0.12 ¢ on the
  needle (the estimate wobbles up to 2.22 ¢), and ≤ 0.92 ¢ at 10 dB SNR.
- The guard survives: the leap sweep, the downward changes, the detuned and pinch cases show no
  judged wrong frame on the needle, and chords no third note. On every recording the needle shows
  no gross frame and never **flickers** (blank while the estimate shows the note the needle showed
  last, within a gap it outlives). Before the octave rule, the guard settling held notes' octaves
  blanked it 2–27 times a recording, in six of them (`ukulele_e4` 27).
- **The lock and the needle can disagree for M3 to settle.** After an onset the needle approaches
  from up to 3 ¢ off and then at the filter's rest time constant (265 ms). At a 1 ¢ tolerance the
  lock, read from `frequency_raw`, may come on while the needle still reads outside it. Which value
  drives `in tune` and `locked` on screen is M3's decision.
- A tuner that shows the note's name needs the note table (`T-003d`) before lock can run in the
  engine (M3). Until then it is tested on cents from the true synthetic frequency.
- The bound ties the needle to the median's speed on a fast change, by design. A peg turned at
  100 ¢/s trails by 1.7 ¢, under the bound, so the filter shapes it.

## Options rejected

- **Faster filter parameters** (`beta` 0.1–1, `d_cutoff` 10–30 Hz): they meet the ledger, but
  wobble 2–6 ¢ on a noisy held note, failing §1's stability row.
- **Re-seeding the filter at the median past a threshold**: it meets both only between 4 and 5 ¢,
  and it jumps the needle.
- **Widening the smoothing line** (`adr/0028`'s ledger): it spends 19 ms of the 190 ms target on
  an artefact of the filter's state, not on information.
- **A median across a note change or a `None`** (a plain rolling median): for two hops it shows
  the old note, or a mix of the two octaves the guard kept apart.
- **Starting a new note on any octave change** (this ADR's first draft): the needle blanked for
  the guard's frames plus a warm-up each time it settled a held note's octave. That breaks the
  owner's decision of 2026-10-23 that an `ambiguous octave` frame moves the needle.
- **Resetting on a single frame of another note:** one stray attack frame cost the held note a
  warm-up (`nylon_e2`).
- **Note math in `dsp` for the lock**: it would duplicate `T-003d`'s note table before it exists.
