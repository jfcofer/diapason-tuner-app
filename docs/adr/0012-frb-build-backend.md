# 0012 — Keep cargokit; native assets is not yet viable for this app

**Status:** Accepted · 2026-08-26 · confirms [0003](0003-flutter-rust-bridge.md)

## Context

`adr/0003` chose flutter_rust_bridge v2 + **cargokit** on 2026-08-25. On 2026-08-23 FRB 2.13.0
shipped a **Native Assets** backend built on Dart build hooks, stable since Flutter 3.38 / Dart 3.10
and described by FRB's own docs as "possibly the preferred way in the future". Cargokit is still
FRB's default.

This is the least reversible decision in `T-001`, so it was decided by building both and measuring
(`T-001a`), not by reading. The full result table is in the task file.

## Decision

**Cargokit stays.** `adr/0003` is confirmed, not superseded.

Native assets is genuinely the nicer artefact: **24 files in the package against cargokit's 66**, no
vendored build tooling at all against cargokit's 35 vendored files, and one seven-line
`hook/build.dart` in place of `android/`, `ios/`, `linux/`, `macos/` and `windows/`. On the criteria
`T-001a` set in advance it either tied or won.

It loses on something the criteria did not anticipate. **The native-assets package has no `ios/`
directory**, because it has no platform-code concept at all — the hook produces a code asset and
Flutter bundles it. `docs/PLATFORM_AUDIO.md` §3 requires **AVAudioSession to be configured from
Swift**, with interruption and route-change subscriptions, explicitly *not* owned by the Rust side.
Under native assets that Swift has nowhere to live, so `packages/audio_engine` would have to split
into a code-asset package plus a separate conventional plugin for the session — inventing a package
boundary `AGENTS.md` §5 does not have, to save build glue we would then partly reintroduce.

Everything else was close enough not to decide it:

- **The decisive criterion tied.** `oboe` links under both. `oboe-sys` compiles Oboe's C++ through
  the NDK in each case, verified by finding oboe symbols in the shipped `.so`.
- **16 KB alignment tied** — `LOAD align 0x4000` under both, which also confirms empirically that
  NDK r28+ aligns by default (`adr/0011`).
- **Iteration cost tied** — 6 s vs 5 s to rebuild after a Rust edit.

Two smaller native-assets frictions, neither disqualifying: it generates its own
`rust-toolchain.toml` pinning an **exact** Rust version (1.93.1, older than our stable 1.98.0)
because `native_toolchain_rust` requires one for reproducible builds — overridable to 1.98.0, which
we verified. And it rejects `--platforms` filtering, because Flutter's `package_ffi` template does.

## Consequences

**Good.** No change to `adr/0003`, `docs/REPO_LAYOUT.md` or the `T-001` plan. Cargokit's Gradle and
Xcode hooks are the proven path, and `packages/audio_engine/ios/Classes/` is the natural home for
the AVAudioSession code `PLATFORM_AUDIO.md` already specifies. One package, one boundary.

**Bad, and we should be honest about it.** We are keeping 35 vendored files of someone else's build
tooling, which is a real maintenance liability: cargokit's `build_tool` carries its own Dart
dependency tree, and on first resolve pub reported **15 of its 25 dependencies already pinned below
their current versions**. That tree ages independently of us and we do not control it. We are also
choosing the backend the upstream project describes as the past tense.

**This is a "not yet", not a "no".** Revisit when either becomes true:

1. The FRB native-assets backend, or Flutter's build hooks, grow a platform-code story — so Swift
   and Kotlin glue can live beside the code asset in one package.
2. We end up moving platform glue down into `core_platform` for other reasons, at which point
   `audio_engine` becomes pure code asset and the objection above evaporates.

`T-001a`'s criterion 4 (iOS archives on a macOS runner) is still **unverified** — there was no macOS
runner when this was decided, and no macOS host in the project. If cargokit fails to archive for
iOS on CI, this ADR is the one to reopen, and native assets is the standing alternative.

**Rejected: adopting native assets now and adding a second plugin package for Swift.** It trades a
build-system simplification for an architectural complication, in the layer `AGENTS.md` §5 is most
concerned with keeping clean. Wrong direction.

**Rejected: deciding from the FRB docs' own recommendation.** The docs hedge ("possibly the
preferred way in the future"), and the thing that actually decided it — no `ios/` directory — is
not something the docs frame as a tradeoff at all.
