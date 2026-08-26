# Product specification

This document owns **what the app does** — features, states, edge cases, and explicit non-goals.
It does not describe how anything is built.

## 1. Positioning

A tuner and metronome that a working musician trusts and a beginner is not intimidated by. The
competition is either accurate and ugly, or pretty and imprecise, or free and full of ads. The
differentiators, in priority order:

1. **Accuracy you can see** — a strobe display that physically stalls at pitch, not a needle that
   guesses.
2. **Zero friction** — opens straight into the tuner, listening, in under a second. No account, no
   onboarding wall, no interstitial.
3. **Nothing leaves the device** — no network permission, no analytics, no ads. Stated plainly on
   the store listing.
4. **It works for the instruments people actually own**, and speaks their note names (letters,
   fixed-do solfège, or German H).

## 2. Tuner

**Core loop.** Open → it is already listening → play a note → see the note name, the cents
deviation, and the strobe → adjust → it locks with a colour change, a settling animation, and an
optional haptic tick.

**Modes**
- *Chromatic* (default) — nearest note of the current temperament.
- *Instrument preset* — snaps to the strings of a chosen tuning, shows all strings at once with
  per-string state, and highlights the one being played. Presets: guitar (E-standard, drop D, drop
  C, DADGAD, open D/G/E, half-step down), 7- and 8-string, bass 4/5/6, ukulele (GCEA, ADF#B, baritone),
  violin, viola, cello, double bass, mandolin, banjo (open G, double C), and user-defined.
- *Manual target* — pick a note, tune to it. For non-standard instruments.

**Settings that affect detection**
Reference A4 (415–466 Hz, 0.1 Hz steps, default 440), temperament (equal, just-in-key, Pythagorean,
Werckmeister III, Kirnberger III, Vallotti, sweetened guitar), transposition (±12 semitones),
per-string offsets, in-tune tolerance (±1 to ±10 cents, default ±3), note-name system (English,
fixed-do, German), and pitch display (cents / frequency / both).

**States the UI must handle explicitly** — and none may look like a bug:
`listening` (no signal above gate) · `detecting` (signal, low clarity) · `flat` · `sharp` ·
`in tune` · `locked` (held in tune) · `too quiet` · `too loud / clipping` ·
`ambiguous octave` · `permission denied` · `mic unavailable`.

**Edge cases with defined behaviour**
Very low notes take longer to settle — show it as intent (a slower, deliberate settle), not as lag.
Harmonics and pinch harmonics may read an octave up: the octave guard handles it, and the display
never flickers between octaves. Ambient noise raises the gate rather than producing wrong readings.
Two strings ringing at once: hold the last confident estimate rather than alternating.

## 3. Metronome

**Core loop.** Tap a tempo or drag the dial → hit play → hear and see the beat.

**Features**
- 20–400 BPM, integer steps, plus tap tempo with outlier rejection (median of the last 5 intervals,
  discard anything > 40 % from the median).
- Arbitrary time signatures, numerator 1–32, denominator 1/2/4/8/16.
- Subdivisions: quarters, eighths, triplets, sixteenths, swing (adjustable ratio), dotted patterns.
- Per-beat accent editing: each beat cycles silent → normal → accent by tapping it.
- Count-in of N bars before recording/practice.
- **Tempo trainer** — increase by X BPM every N bars, optionally with an upper bound and a return.
- **Gap trainer** — play N bars, mute M bars, to test internal time. The single most requested
  serious-practice feature and the cheapest to build once scheduling is exact.
- Presets/setlists: named tempo + signature + subdivision combinations. Post-1.0: ordered setlists
  with per-song notes.
- Sound: three procedural timbres (wood, digital, soft) × accent/beat/subdivision, with volume per
  layer. No sample packs in 1.0 (`AUDIO_ENGINE.md` §5).
- Haptic mode: vibrate on beats, with or without sound, locked to the audio clock.
- Background playback with lock-screen transport controls.

**Edge cases** — tempo changed while playing (takes effect at the next bar, phase preserved),
signature changed while playing (applies at the next bar), extreme tempi (at 400 BPM with
sixteenths, subdivisions merge visually rather than strobing), device muted (a visible warning, not
silence the user cannot explain), and screen off (must keep exact time).

## 4. Shared

- **Instant resume**: the app returns to the last screen and settings, always.
- **Settings**: theme (system/light/dark), language (English/Spanish at launch), haptics, keep
  screen awake, latency calibration, tolerance, note-name system, reset to defaults.
- **Accessibility is a launch requirement**, not a follow-up: full screen-reader labels including a
  live-region announcement of the tuning state, dynamic type support up to the largest system size
  without clipping, reduced-motion honoured (strobe becomes a discrete indicator), minimum 4.5:1
  contrast, 48 dp touch targets, and **never colour alone** — in-tune is signalled by colour *and*
  the strobe stalling *and* a shape change *and* optional haptics, so a colour-blind user has three
  other channels.
- **Localisation** covers note names as a first-class concern. In much of Latin America and southern
  Europe notes are Do–Re–Mi, not C–D–E; getting this wrong makes the app feel foreign to a large
  part of the guitar-playing world. German speakers expect H for B♮.

## 5. Non-goals for 1.0

Polyphonic/chord tuning · recording or playback of takes · chord libraries or lesson content ·
cloud sync or accounts · a watch app · MIDI · audio interface / multi-channel input ·
sample-based click packs · ads or subscriptions. Each of these is a reasonable future; none may
influence the 1.0 architecture beyond leaving the seams that already exist.

## 6. Monetisation (decide before M6, not now)

The design assumption is one-time purchase or a small free tier with a paid unlock for the trainer
features. **No advertising and no data collection under any model** — that constraint is
architectural, and reversing it later means rewriting the privacy story, the store listing, and the
network posture of the app. Record the decision as an ADR when it is made.

## 7. Quality bar for "done"

A feature is finished when: it works on the four reference devices, it handles every state in its
section above, it has tests per `TESTING.md`, it is accessible per §4, it is localised in both
launch languages, it looks right at 320 dp and at 1024 dp, it survives the lifecycle matrix in
`PLATFORM_AUDIO.md` §5, and it does not regress a budget in `AUDIO_ENGINE.md` §1.
