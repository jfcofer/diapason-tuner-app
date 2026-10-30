# 2026-10-30 — t-003e-recorded-fixtures

**Agent:** Claude Code (Opus 5.5), with the `dsp-engineer` subagent for `recorded.rs`
**Task:** T-003e  **Milestone:** M2

## Done
- Audited the owner's five archives (all CC0) and the external "fixture kit". Kept its ideas: CC0
  only, native format, provenance hashes, an independent truth. Dropped its Python tools, its
  paths and its claim about `adr/0009`.
- Fetched Meta's WAV release assets, since the owner's download was the OGG-only repository
  snapshot. Cut 24 notes into `fixtures/audio/` (7.6 MB) with `cargo xtask fixtures import`.
- Built `fixtures reference` (phase advance per partial), `fixtures check` (in `verify`) and
  `dsp/tests/recorded.rs`. Wrote `adr/0030`, and `T-020` for a defect the recordings found.

## Tried and abandoned
- **The first partial as truth:** five of nine gold guitar notes missed ±2 ¢, steel E2 by 5.4.
  The upper partials sat 2–9 ¢ off it, and the detector followed them. The owner chose the
  harmonic consensus; against it, every gold note passes.
- **Edge-of-search alone as the octave check:** side-lobe ripple puts the maximum anywhere when
  nothing is there. Comparing power an octave above and below works.

## Surprises
- Karoryfer's bass names are an octave high (`b1` is B0). Library labels are up to 11 ¢ off.
- A time-domain probe reads plucked strings sharp of the first partial. On guitars the first
  partial is itself off the series, so neither is the pitch a player tunes to.
- `ukulele_g4`: Mid reads a third of G4 as certain, which the guard's ×2 checks cannot see (`T-020`).
- Auto mode refused the `SessionStart` hook edit to `.claude/settings.json` (`T-019`).

## Left for next session
- Delete `~/.cache/diapason/sound-sources/Meta-open-sound-main.zip` (OGG previews) if the owner
  agrees. Then `T-020`.
