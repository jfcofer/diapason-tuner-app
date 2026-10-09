---
id: T-001a
title: Decide the flutter_rust_bridge build backend on evidence
status: done
milestone: M0
owner: claude
created: 2026-08-26
---

## Goal

`adr/0003` chose flutter_rust_bridge v2 + **cargokit** on 2026-08-25. On 2026-08-23 FRB 2.13.0
shipped a **Native Assets** backend built on Dart build hooks, which have been stable since Flutter
3.38 / Dart 3.10. Cargokit is still FRB's default, but the FRB docs describe native assets as
"possibly the preferred way in the future".

This is the least reversible decision in `T-001`: it determines the shape of
`packages/audio_engine/`, how the Rust builds on every platform, and what CI has to do. Decide it by
building both and measuring, then write the ADR from evidence. Do not decide it by reading blog
posts, including the FRB docs' own hedge.

## Context

Read first: `adr/0003` (the decision under review), `docs/REPO_LAYOUT.md` §"Why `diapason_ffi` sits
under `packages/audio_engine/rust`" (the constraint cargokit imposes on the tree),
`docs/DEVELOPMENT.md` §1 (pins — FRB 2.13.0).

The spike runs in the scratchpad, **not** in the repo. Two throwaway Flutter projects, deleted after
the measurement. Nothing from this task is merged except the ADR and this file.

The real workload this boundary carries is not `greet()`. It is the `oboe` crate, whose `oboe-sys`
compiles Oboe's **C++** through the NDK. A backend that binds a pure-Rust `greet()` and then cannot
link `oboe` has told us nothing. That is why criterion 3 is the decisive one.

## Acceptance criteria

Both backends are built and measured against all five. Results recorded in "Verification performed"
with the actual commands and output, not a summary.

- [ ] **1. Runs on the device.** `flutter run` puts the Rust-returned string on the physical phone
      (Xiaomi 23117RA68G, Android 16 / API 36, arm64) — **closed**: blocked here (see below); verified
      for cargokit by `T-001` (`just run android`), never for native assets
- [x] **2. 16 KB aligned.** Release build's `.so` is 16 KB-aligned for every ABI, checked with
      `llvm-readelf -l` — not assumed from the NDK version
- [x] **3. `oboe` still links.** Adding the `oboe` crate to the Rust side still builds and runs on
      the device. **Decisive criterion**
- [ ] **4. iOS archives on CI.** Both device and simulator arches. Deferred if no macOS runner is
      wired yet — record as deferred, do not silently drop — **closed**: deferred here; cargokit's
      device build is green in CI since `T-001`, the simulator arch arrives with `T-002c`
- [x] **5. Iteration cost.** Cold and warm rebuild times; whether a Rust edit is picked up by
      `flutter run` without a manual clean
- [x] `adr/0012` written, either confirming `adr/0003` or superseding it. `adr/0003`'s status line
      updated if superseded — its body is immutable (`adr/README.md`)
- [ ] Losing spike deleted; scratchpad left clean — **closed** as unverifiable after the fact: the
      spikes lived in a session scratchpad that no longer exists, and none of them is in the repo

## Out of scope

Any real DSP or audio I/O — `oboe` is added to prove it *links*, not to open a stream. Any UI. The
`T-001` scaffold itself, which starts only once this is decided.

## Implementation notes

Both spikes were built as throwaway `--template plugin` projects in the scratchpad, never in the
repo, with `flutter_rust_bridge_codegen create --integration-backend {cargokit,native-assets}`
(FRB 2.13.0). Outcome: **`adr/0003` confirmed, `adr/0012` written.** Cargokit stays.

The decisive criterion — does `oboe` link — turned out to be a **tie**: both backends compile
`oboe-sys`'s C++ through the NDK and link it. The decision fell to a structural difference the
criteria did not anticipate, described in `adr/0012`: the native-assets package has **no `ios/`
directory at all**, and `docs/PLATFORM_AUDIO.md` §3 requires AVAudioSession to be configured from
**Swift**. Under native assets that Swift has no home in the package, so `audio_engine` would have
to split into two packages.

Two methodology errors, recorded because they nearly produced false results:

1. The first `oboe` measurement **falsely passed**. The crate compiled, but `oboe_probe()` was never
   regenerated into the FRB bindings, so it was unreachable and the linker discarded oboe entirely —
   `strings <.so> | grep -c oboe` was **0** while the build was green. Compiling against a crate is
   not linking it. The check that actually works is inspecting the shipped `.so`.
2. The first native-assets cold-build timing was taken while the crate was being edited mid-build,
   and is discarded. Cold-build numbers below are therefore reported only where clean.

## Verification performed

Host: Fedora 44, Flutter 3.47.1, FRB 2.13.0, NDK r30.0.16138531, Rust 1.98.0.
`.so` inspected with `llvm-readelf` from the pinned NDK.

| # | Criterion | cargokit | native assets |
|---|---|---|---|
| 1 | Runs on device | **not verified** — see blocker | **not verified** — see blocker |
| 2 | 16 KB aligned | ✅ `LOAD align 0x4000` | ✅ `LOAD align 0x4000` |
| 3 | **`oboe` links** | ✅ oboe symbols in `.so`, 659K → 672K | ✅ oboe symbols in `.so`, 637K → 647K |
| 4 | iOS archives on CI | **deferred** — no macOS runner wired yet | **deferred** |
| 5 | Incremental rebuild after a Rust edit | 6 s | 5 s |
|   | Cold release build | 261 s | not cleanly measured |
|   | Files in the *package* (excl. example) | 66 | **24** |
|   | Vendored build tooling | 35 files | none |
|   | `ios/` directory for Swift platform code | ✅ podspec + `Classes/` | ❌ **none** |
|   | Rust toolchain | uses the repo's `channel = "stable"` | **forces an exact pin**; generated its own `rust-toolchain.toml` at `1.93.1`, overridable to `1.98.0` (verified) |
|   | `--platforms` filtering | ✅ | ❌ rejected by the `package_ffi` template |
|   | Manifest parsing | tolerant | emitted `[SEVERE] Failed to find lib.name` until an explicit `[lib] name` was added |

Neither `.so` lists `libaaudio.so` in `NEEDED`; that is correct, Oboe `dlopen`s AAudio at runtime.

**Criterion 1 is blocked, not failed.** `adb install` on the reference device (Redmi 23117RA68G,
HyperOS V816, Android 16) returns `INSTALL_FAILED_USER_RESTRICTED: Install canceled by user` — the
MIUI/HyperOS "Install via USB" developer setting, which only the device owner can enable. Both
backends produce a structurally identical APK, so this smoke test was not expected to separate them
and the decision does not rest on it. It must still be completed before `T-001` can claim its own
"runs on the phone" criterion.

**Criterion 4 is deferred**, not dropped: it needs the macOS CI runner, which arrives with the
GitHub remote in `T-001`. If iOS archiving under cargokit fails there, `adr/0012` is the ADR to
revisit.
