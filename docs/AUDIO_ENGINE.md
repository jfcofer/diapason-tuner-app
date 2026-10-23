# Audio engine

This document owns the **algorithms and real-time contracts** inside `rust/crates/{dsp,engine}`:
how pitch is detected, how beats are scheduled, and the numbers any implementation must hit.
Platform I/O configuration is in `PLATFORM_AUDIO.md`.

## 1. Performance budgets

These are acceptance criteria, not aspirations. `just bench` asserts the DSP ones; the end-to-end
ones are measured on the reference devices in `TESTING.md` §7.

| Budget | Target | Fails at |
|---|---|---|
| Perceived tuner latency (string plucked → needle moves) | ≤ 60 ms for f₀ ≥ 80 Hz | > 100 ms |
| Same, for bass low B (≈ 31 Hz) | ≤ 140 ms | > 220 ms |
| Pitch accuracy, synthetic steady tone 30–1400 Hz | ±0.5 cent | ±1 cent |
| Pitch accuracy, recorded acoustic guitar fixtures | ±2 cents | ±4 cents |
| Display stability, held note | ≤ 1 cent peak-to-peak wobble | visible flutter |
| Metronome inter-onset jitter | 0 samples (exact) | any drift over 30 min |
| Analysis frame cost (4096-pt NSDF) | ≤ 1.5 ms on a 2021 mid-range phone | > 4 ms |
| CPU while tuning | ≤ 8 % of one core | > 15 % |
| Audio callback worst case | ≤ 15 % of one burst's period (`adr/0025`) | > 40 % |
| Added binary size per ABI | ≤ 2.5 MB | > 4 MB |

**Latency ledger.** Plucked string to moving needle, worst case, stage by stage, in ms at 48 kHz
(44.1 kHz is within 0.3 ms). The constants live in `diapason_dsp::range` (`adr/0028`);
`tests/chain.rs`, `onset_latency_is_within_the_ledger`, asserts each range's chain line at both
rates, from silence to the first estimate within ±5 ¢ that stays, on the audio clock.

| Stage | Low (from 30 Hz) | Mid (from 75 Hz) | High (from 160 Hz) |
|---|---|---|---|
| Input I/O, provisional until `T-002f` (one 960-frame burst) | 20 | 20 | 20 |
| Filter group delay (129 or 29 samples) | 2.7 | 2.7 | 0.6 |
| Window fill (three periods of the floor) | 100 | 40 | 18.75 |
| Settling (the DC block's transient, measured) | 15 | 5 | 1 |
| One hop, waiting for the next frame | 5 | 2.5 | 2.5 |
| **Chain line, asserted** (measured worst, both rates) | **122.7** (117.1) | **50.2** (49.1) | **22.9** (21.3) |
| Smoothing reserve: two hops for `T-003c`'s median of five | 10 | 5 | 5 |
| Snapshot period (30 Hz) | 33.3 | 33.3 | 33.3 |
| One display frame (60 Hz) | 16.7 | 16.7 | 16.7 |
| **Total** | **202.7** | **125.2** | **97.9** |

**These totals miss the latency budgets above** (`T-003b`, Deviation 1): 60 ms for f₀ ≥ 80 Hz and
140 ms for low B. I/O, the snapshot and the display alone sum to 70 ms before any analysis, so no
detector meets 60 ms under this ledger. Moving the budgets, the snapshot cadence or the ledger's
worst-case convention is the owner's decision; until then the budget rows stand, unmet.

## 2. Signal chain (tuner)

```
input ──▶ DC block ──▶ level/gate ──▶ ring buffer ──▶ [analysis thread]
                                                          │
            ┌── f₀ < 160 Hz: decimate ×4 (anti-aliased) ◀─┤
            └── f₀ ≥ 160 Hz: band-limit at full rate ◀────┘
        │
        ▼
   frames per range ──▶ NSDF via FFT autocorrelation ──▶ peak pick ──▶ parabolic interp
        │                                                                     │
        └──▶ clarity ──▶ octave guard ──▶ median(5) ──▶ one-euro filter ──▶ snapshot
```

**Pre-processing.** One-pole high-pass at 20 Hz removes DC and handling rumble (`dsp::filter::
DcBlock`). An RMS gate (default −50 dBFS, adaptive to the noise floor measured over the last 2 s)
decides when nothing is being played — a tuner that hunts in silence feels broken. Its floor
learns only while it is closed, or a held note would become the floor and close it, and it never
mutes the audio the detector sees, only whether an estimate is shown (`dsp::gate::NoiseGate`).

**Band-limiting and decimation.** Parabolic interpolation is biased by harmonics near the
*analysis* Nyquist (`T-003a`), so both paths keep their content below an eighth of their analysis
rate, through linear-phase Kaiser FIRs designed per rate (`dsp::range::RangeTable`). The Low and
Mid ranges analyse at a quarter of the input rate (12 kHz from 48 kHz, 11.025 kHz from 44.1 kHz),
which cuts their NSDF cost about 4×. High analyses at the full rate. The crossover sits at the
Mid/High boundary, 160 Hz. That is not because the decimated path fails above it, since it keeps
a 2× margin to 605 Hz in Mid's window. It is because a window as short as High's has too few
samples at the decimated rate. Range changes are hysteretic (a semitone) to avoid oscillating at a
boundary (`dsp::range::RangeSelector`).

**Window sizing is adaptive**, because a fixed window either misses low strings or is needlessly
slow for high ones. A window holds three periods of its range's floor, and the hop is short,
because each hop is paid three times in latency (§1, `adr/0028`):

| Range | Floor | Analysis rate | Window @ 48 / 44.1 kHz | Hop | Update rate |
|---|---|---|---|---|---|
| Low (5-string bass B to the D string) | 30 Hz | ÷ 4 | 1200 / 1103 (100 ms) | 5 ms | 200 Hz |
| Mid (guitar low E, bass upper) | 75 Hz | ÷ 4 | 480 / 441 (40 ms) | 2.5 ms | 400 Hz |
| High (guitar D3 and up, violin, uke) | 160 Hz | full | 900 / 827 (18.75 ms) | 2.5 ms | 400 Hz |

*Note.* The windows are short because a window that is only partly filled with a new note biases
the NSDF, so the first good estimate waits for the window to fill (§1's ledger). The selector
returns to the home range on silence. The instrument preset picks that range (M3), so the common
case (guitar E-standard, Mid) never pays the bass window. A note the home range cannot see (1.4 kHz
from Low is in the decimator's stopband) is found by scanning to the next range after a window and
50 ms of unclear frames. Every range's history is kept full, so a switch costs one hop.

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
   frequency. This is what buys sub-cent resolution from a coarse lag grid.
6. **Clarity** = interpolated NSDF value at the peak. Below 0.6 the estimate is discarded and the
   UI shows "listening" rather than a wrong note.

**Octave guard.** The most visible failure mode of any tuner. Two defences: (a) before accepting a
peak, test whether a peak of comparable clarity exists at τ/2 or 2τ and prefer the one consistent
with the previous accepted estimate; (b) hysteresis — a jump of more than 600 cents from a
currently-locked estimate requires three consecutive confirming frames.

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
