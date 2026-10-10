# 0021 — Release builds abort on panic; the FFI surface is panic-free by construction

**Status:** Accepted · 2026-10-09

## Context

`ARCHITECTURE.md` §7 and `AGENTS.md` §6 said the FFI boundary wraps every entry point in
`catch_unwind`, so that "a panic becomes an error rather than a process abort". Two facts make that
false in every build that ships:

- **The workspace's `[profile.release]` sets `panic = "abort"`.** A panic aborts the process before
  any `catch_unwind` runs. flutter_rust_bridge 2.13.0 does wrap its handlers in `catch_unwind`
  (`src/handler/implementation/`), but that only takes effect in debug builds.
- **The AAudio callbacks are `unsafe extern "C" fn`** (`audio_io/src/android.rs`). Since Rust 1.81
  a panic that reaches an `extern "C"` boundary aborts whatever the panic strategy. The RT path
  could never have been caught.

So the documented guarantee was never true. The question was which way to make the docs and the
code agree.

## Decision

**Keep `panic = "abort"` for release builds.** A panic is a bug, and it ends the process loudly.
The OS keeps its own crash record (a tombstone on Android, a crash log on iOS). We send nothing
(`adr/0010`).

**Make "no panics" a checked property rather than a hope.** The hand-written FFI surface denies the
panicking constructs at compile time: `unwrap_used`, `expect_used`, `panic`, `indexing_slicing`,
`unreachable`, `todo` and `unimplemented`. Every fallible step returns a `Result` that reaches Dart
as an error. New non-RT crates that the FFI calls into, such as the stream supervisor of `T-002b`,
adopt the same denials.

The denials attach to `pub mod api` in `diapason_ffi`'s `lib.rs`, not to the crate's `[lints]`
table. flutter_rust_bridge's generated `frb_generated.rs` lives in the same crate and unwraps
inside its own wire decoder. That code is generated, not ours to judge (`docs-check` already
excludes it), and its input is produced by flutter_rust_bridge's own Dart encoder.

`dsp` and `engine` are panic-free on their RT paths by design and review: `processor.rs` and the
`dsp` types document it, and no test proves it. The real-time rules in `AGENTS.md` §6 are
unchanged.

## Options rejected

- **`panic = "unwind"` in release, so FRB turns panics into Dart exceptions.**
  - It protects only the synchronous FFI calls. A panic on the RT thread still aborts at the
    `extern "C"` boundary.
  - A panic on a background Rust thread (the stream supervisor) would kill that thread silently.
    Dart would keep waiting on a session that no longer exists, a worse failure than a crash.
  - It also brings back the unwinding landing pads that `abort` leaves out.
- **Keep the documentation and add nothing.** That leaves a promise the build cannot keep. It is
  the drift this ADR exists to remove.

## Consequences

- `ARCHITECTURE.md` §7 and `AGENTS.md` §6 now cite this ADR instead of `catch_unwind`.
- Errors the UI can act on (`PermissionDenied`, `DeviceUnavailable`, …) must be `Result`s all the
  way up. A panic is never an error-reporting path.
- Debug builds still unwind, so a panic in a debug FFI call shows up as a Dart exception during
  development. Release behaviour differs on purpose, and only for code that is already a bug.
- **Revisit** if a crate the FFI calls must run third-party code that can panic. That code would
  need its own isolation, not a global strategy change.
