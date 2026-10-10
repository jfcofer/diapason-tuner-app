# Architecture Decision Records

An ADR records a decision that **closes off an alternative**, together with the reasoning and the
cost. Code says what we do; an ADR says what we rejected and why — which is what stops the next
agent from "improving" a deliberate choice.

## Rules

- One decision per file, `NNNN-kebab-title.md`, numbered sequentially and never reused.
- Status: `Proposed` → `Accepted` → `Superseded by NNNN` (or `Rejected`).
- **Accepted ADRs are immutable.** To change a decision, write a new ADR that supersedes it and
  update the old one's status line only.
- Budget: 120 lines. If it needs more, the decision is not yet clear enough to record.
- Write one when: choosing between libraries or architectures, accepting a constraint that will be
  expensive to reverse, or deliberately doing something that looks wrong.

## Index

| # | Title | Status |
|---|---|---|
| [0001](0001-rust-owns-audio.md) | Rust owns the entire audio path | Accepted |
| [0002](0002-agent-context-system.md) | AGENTS.md as canonical contract, tiered memory | Accepted |
| [0003](0003-flutter-rust-bridge.md) | flutter_rust_bridge v2 + cargokit for the FFI boundary | Accepted |
| [0004](0004-state-management.md) | Riverpod 3 for discrete state, Listenable for continuous | Accepted |
| [0005](0005-pitch-detection-algorithm.md) | MPM/NSDF for pitch detection | Accepted |
| [0006](0006-monorepo-tooling.md) | Pub workspaces + Melos | Accepted |
| [0007](0007-domain-model-duplication.md) | Duplicate note math in Rust and Dart | Accepted |
| [0008](0008-procedural-click.md) | Synthesise metronome clicks, ship no audio assets | Accepted |
| [0009](0009-licensing-and-third-party.md) | Licensing and dependency policy | Accepted |
| [0010](0010-no-telemetry.md) | No telemetry, no network calls | Accepted |
| [0011](0011-toolchain-pins.md) | Re-pin the toolchain, make pins machine-checkable | Accepted |
| [0012](0012-frb-build-backend.md) | Keep cargokit; native assets not yet viable | Accepted |
| [0013](0013-name-and-bundle-id.md) | Locale-aware name, ASCII identifiers, `dev.jfcofer.diapason` | Accepted |
| [0014](0014-drop-custom-lint.md) | Drop custom_lint for first-party analyzer plugins | Accepted |
| [0015](0015-pin-rust-toolchain.md) | Pin the Rust toolchain to an exact release | Accepted |
| [0016](0016-goldens-environment.md) | Goldens on the pinned Ubuntu runner, no third-party container | Accepted |
| [0017](0017-project-licence.md) | Project licence: FSL-1.1-ALv2 (Apache-2.0 after two years) | Accepted |
| [0018](0018-lossy-numeric-conversions.md) | Lossy numeric conversions live in one audited module | Accepted |
| [0019](0019-min-sdk-28.md) | Raise Android minSdk to 28 | Accepted |
| [0020](0020-android-audio-binding.md) | Android audio: AAudio through raw `ndk-sys`, with our own wrapper | Accepted |
| [0021](0021-panic-abort.md) | Release builds abort on panic; the FFI surface is panic-free by construction | Accepted |
| [0022](0022-stream-supervisor-crate.md) | A `session` crate supervises the stream and replays desired state | Accepted |
| [0023](0023-kotlin-formatting.md) | Format Kotlin with a pinned ktfmt, kotlinlang style | Accepted |
| [0024](0024-platform-channels-and-device-capabilities.md) | Pigeon platform channels; Rust picks the preset; the session is told the permission | Accepted |
