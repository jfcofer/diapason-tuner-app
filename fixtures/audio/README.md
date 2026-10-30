# Recorded fixtures

Real instrument notes, cut from CC0 sample libraries and each held to an independently measured
pitch. The rules (licences, format, size caps, what counts as truth) are `docs/adr/0030`. This file
is how to work with them. Synthetic fixtures are generated in code and never live here
(`docs/adr/0027`).

## What is here

- `*.wav`: one channel of the source, at its own rate and depth, cut on frame boundaries.
- `manifest.json`: per library, the URL, version, archive SHA-256 and licence. Per note: the
  archive member and its SHA-256, the channel, the cut, the set, the note *as measured*, and the
  reference pitch with its window, method and uncertainty. The schema lives in
  `rust/crates/xtask/src/fixtures/manifest.rs`.
- `LICENSES/`: each library's licence file, copied verbatim from its archive (CRLF included:
  `.gitattributes` keeps the bytes, and the manifest pins their SHA-256).

Sets: **gold** is held to `AUDIO_ENGINE.md` §1 (acoustic guitar, ±2 cents). **ceiling** is held
under a regression ceiling until `T-003g` sets a budget. **stress** is reported only.

## Commands

```bash
cargo xtask fixtures check                       # in `just verify`
cargo xtask fixtures import --id steel_e2 --source meta-steel-guitar \
  --archive ~/.cache/diapason/sound-sources/meta-open-sound-meta-steel-guitar-wav.zip \
  --member INSGuitarSteel_StringLowE_Vel02_E2_01.wav --channel left \
  --start-ms 0 --end-ms 2500 --instrument acoustic-guitar --variant steel --set gold --note E2
cargo xtask fixtures reference --id steel_e2     # window 300–1300 ms unless --window-ms a,b
```

`import` and `reference` rewrite `manifest.json`. A new cut drops its old reference, unless the
bytes are the same.

## Adding a note

1. Download the library's **release archive**. A repository snapshot may hold only lossy
   previews: Meta Open Sound's has OGGs, not WAVs. Keep archives outside the repo, in
   `~/.cache/diapason/sound-sources/` by convention.
2. A new library: confirm its licence from the file inside the archive, then copy that file into
   `LICENSES/`. Add a `sources` entry with `archive_sha256` empty; the first import fills it.
3. Choose the note **by measurement, not by its file name**, and audition the channels.
4. `import`, then `reference`. If `reference` refuses because the label is an octave or a
   semitone off, fix `--note`. If a gold note's uncertainty exceeds 0.5 cents, move the window
   later, where the string has settled, or make it a ceiling note.
5. `check`, then `cargo nextest run -p diapason_dsp --test recorded`.

## Traps

- **Karoryfer bass names are an octave high.** `darkblack_b1_*` is B0 (30.87 Hz), `darkblack_e2_*`
  is E1. `babyblue/fake_*` is pitch-shifted or a string tuned down: never use it.
- **Note names are not pitch.** Notes here sit up to ±11 cents from nominal.
- **The first partial is not the pitch.** A guitar's upper partials sit up to 9 cents from it,
  and the body pulls it up to 3.6 cents from the series the ear follows (4.5 on the bass). The
  reference is the harmonic consensus of partials 1–6; `first_partial_hz` is kept for comparison.
- Emilyguitar samples every third semitone from A2 (`a2`, `c3`, `eb3`, …), plus `db2`, `e2` and
  `gb2` below it: there is no E4.
- VSCO's violin is quiet (about −40 dBFS RMS) and is a stereo pair from two microphones: one
  channel, never a mix.

## Sources

All CC0-1.0. Attribution is not required, and given anyway: Meta Creative Audio (Meta Steel
and Nylon Guitar), Karoryfer Samples (Emilyguitar; Black And Blue Basses), Mateusz Dąbrowski for
FreePats (Ukulele), and Versilian Studios (VSCO 2 Community Edition).
