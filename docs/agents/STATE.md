# STATE — updated 2026-08-26

> The current truth. Rewritten at the end of every session. Budget: 120 lines.
> If you are an agent starting a session: read this, then the active task file, then begin.

## Where we are

**Milestone:** M0 — Foundations (`docs/ROADMAP.md`)
**Status:** The monorepo exists and builds. `just verify` is **green**. A value computed in
`rust/crates/dsp` crosses the FFI boundary and renders on a physical Android device.

## Active task

`docs/agents/tasks/T-001-scaffold.md` — **in progress**, 9 of 12 acceptance criteria met.
The three that are not are all blocked on the same two things: no macOS host, and no git remote.

## The two open blockers

1. **No git remote.** The repo is local-only. CI has never run, which means **no iOS code in this
   repository has ever been compiled**. The user opted to create a private GitHub repo and supply
   the URL; then `git remote add origin <url> && git push -u origin main`.
2. **No macOS host.** `just run ios` and `just setup` on macOS cannot be verified here, by anyone,
   ever. The `build-ios` job on a macOS runner is the only iOS validation this project will get.

## What exists now

- **Pub workspace**, 8 members: `apps/diapason`, `audio_engine`, `core_domain`, `core_platform`,
  `core_ui`, `feature_{tuner,metronome,settings}`. Every one has a real test; all 8 pass.
- **Cargo workspace**, 5 crates: `dsp`, `engine`, `audio_io`, `xtask`, and `diapason_ffi` inside
  the plugin package where cargokit expects it.
- **The gate.** Every `tools/*.sh` the justfile promised now exists. `just verify` runs
  doctor-selftest, fmt, analyze, tests (Dart + Rust + doctests), cargo-deny, codegen-drift,
  dependency-direction and docs checks.
- **Three flavours** installing side by side, with locale-aware launcher labels.

## Decisions already made (do not re-litigate without an ADR)

- Rust owns all audio; Dart never sees a sample. → `adr/0001`
- `AGENTS.md` is canonical; `CLAUDE.md` imports it. → `adr/0002`
- flutter_rust_bridge v2 + **cargokit**, re-confirmed on evidence. → `adr/0003`, `adr/0012`
- Riverpod 3 (code-gen) for discrete state; `Listenable` for continuous. → `adr/0004`
- MPM/NSDF for pitch detection. → `adr/0005`
- Pub workspaces + Melos. → `adr/0006`
- Note math duplicated in Rust and Dart, kept honest by a shared fixture. → `adr/0007`
- Procedural click synthesis, no audio assets in 1.0. → `adr/0008`
- No telemetry, no network calls. → `adr/0010`
- **Toolchain pins live in `tools/versions.env`**, not in prose. → `adr/0011`
- **Name and bundle ID settled**: locale-aware label, ASCII identifiers. → `adr/0013`
- **`custom_lint` removed**; `riverpod_lint` is a first-party analyzer plugin. → `adr/0014`

## Traps a later session will otherwise re-discover

- **JDK 25 breaks Flutter Android builds** ([flutter#187223](https://github.com/flutter/flutter/issues/187223)).
  This machine defaults to 25 and Android Studio bundles 25. Flutter is pointed at JDK 21 via
  `flutter config --jdk-dir`. `just doctor` checks the JDK *Flutter* will use, not the one on
  `PATH`, and names this case explicitly.
- **Compiling against a crate is not linking it.** The first `oboe` measurement in `T-001a` falsely
  passed: the build was green while the linker had discarded oboe entirely, because the probe
  function was never regenerated into the FRB bindings. Verify by inspecting the shipped `.so`.
- **`cargo fmt` reformats FRB's generated file.** Formatting is part of `just gen` for this reason;
  without it `check-drift` oscillates forever.
- **`dart format .` walks build outputs.** cargokit vendors a Dart `build_tool` under
  `apps/*/build/`. Formatting is scoped to `git ls-files`.

## Open questions

| Question | Needed by | Notes |
|---|---|---|
| Licence | M7 | `adr/0009` still Proposed. Crates carry no `license` key; `deny.toml` ignores private crates. Blocks nothing yet |
| Monetisation model | M6 | Must not introduce ads, analytics or network calls (`PRODUCT_SPEC.md` §6) |
| Which four reference devices are physically available | M1 | One known: Redmi 23117RA68G, Android 16, arm64. Budgets are meaningless without the others — especially a cheap one |
| Font licences confirmed for bundling | M3 | `DESIGN_SYSTEM.md` §1 assumes OFL faces |

## Next up (in order)

1. **Finish `T-001`**: push to the remote, get CI green, and fix whatever `build-ios` finds.
   Two iOS items need Xcode on a Mac and were deliberately *not* hand-edited into `project.pbxproj`:
   adding `PrivacyInfo.xcprivacy` to the Runner target's resources (**it will not ship until this
   is done**), and the per-flavour schemes.
2. `T-002-audio-spine` — one duplex stream on both platforms through the real backends.
3. `T-003-pitch-core` — the `dsp` crate against fixtures, offline, no UI.

`docs/BOOTSTRAP.md` is still present; delete it once M0 is complete.
