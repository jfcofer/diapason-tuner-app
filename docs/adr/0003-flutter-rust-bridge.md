# 0003 — flutter_rust_bridge v2 + cargokit for the FFI boundary

**Status:** Accepted · 2026-08-25

## Context

Given ADR-0001, Dart must call Rust and receive a stream of snapshots. Options: hand-written
`dart:ffi` bindings; `flutter_rust_bridge` v2 (FRB); `rinf` (protobuf/signal-based); or a plugin
with method channels over a C API.

The native library must also be *built* for every target as part of the normal Flutter build, which
is a separate problem from binding generation and historically the more painful one.

## Decision

FRB v2 (2.12.x, codegen and runtime pinned to the same version) for bindings, cargokit for the
build integration. `rust/crates/ffi` contains only the API surface; generated bindings are checked
in.

## Consequences

**Good.** Streams, `Result`, enums with payloads, and opaque types map across the boundary without
hand-written glue. cargokit hooks into Gradle and Xcode so `flutter run` builds the Rust for the
right targets without a bespoke script. FRB is a Flutter Favorite with a large user base, so
platform edge cases are usually already solved.

**Bad.** Codegen/runtime version mismatch is the most common failure in this stack — mitigated by a
hard check in `just doctor`. A generated file lands in the diff (deliberately: it is reviewable, and
it keeps codegen off the CI critical path). Some ownership patterns need `#[frb(opaque)]` and
occasional contortion.

**Rejected: hand-written `dart:ffi`.** Fine for five functions, not for an evolving snapshot type
with enums and streams; every change becomes three edits in three languages and an opportunity for
a memory bug.

**Rejected: rinf.** Good design, smaller ecosystem, and its message-passing model fits a
request/response app better than the latest-wins snapshot model this one wants.
