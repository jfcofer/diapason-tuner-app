# 0011 — Re-pin the toolchain, and make the pins machine-checkable

**Status:** Accepted · 2026-08-26

## Context

The pins in `docs/DEVELOPMENT.md` §1 were written on 2026-08-25 as prose, before any code existed.
The first session to actually build anything found four of them wrong or unbuildable:

- **Flutter 3.44.x / Dart 3.12** was pinned. Flutter 3.47.0 went stable on 2026-08-12 and 3.47.1 on
  08-19; stable skipped 3.45 and 3.46 entirely. 3.44 was two releases stale on the day it was
  written down.
- **Java 21** was pinned correctly, but nothing enforced it. Flutter resolves the JDK Gradle uses
  from its own `jdk-dir` setting, then Android Studio's bundled JBR, then `JAVA_HOME` — and on this
  machine every one of those paths led to **JDK 25**, on which Flutter Android builds fail outright
  ([flutter#187223](https://github.com/flutter/flutter/issues/187223), open since 2026-05-28). A
  pin nothing checks is a comment.
- **flutter_rust_bridge 2.12.x** was pinned; 2.13.0 shipped 2026-08-23.
- **Android NDK r27+** was pinned "required for 16 KB page alignment", and `T-001` carried a
  warning that the `-Wl,-z,max-page-size=16384` flag must reach the Rust link step. NDK **r28+
  aligns to 16 KB by default**, which retires that trap rather than mitigating it.

The deeper problem is not the individual numbers. It is that the pins lived in a Markdown table
that no tool read, so nothing could tell the difference between a considered decision and a stale
sentence.

## Decision

**`tools/versions.env` is the single source of truth for every version in the repo.** It is
shell-sourceable, `tools/doctor.sh` enforces it, and CI reads the same file.
`docs/DEVELOPMENT.md` §1 keeps the *reasoning* and explicitly defers to `versions.env` on the
numbers.

Pins as of this ADR: Flutter 3.47.1 / Dart 3.13.1 · Rust stable edition 2024 · FRB 2.13.0 ·
Java 21 (`21.0.12+1.1-tem`) · NDK r30 (`30.0.16138531`) · compile/target SDK 36, min 26 ·
iOS 15.0 · Melos 8.x.

Java 21 is installed alongside, not instead of, the machine's JDK 25, and Flutter is pointed at it
with `flutter config --jdk-dir`. Forcing a global JDK downgrade to fix one toolchain is a bad
trade for anyone who has other work on the machine.

Two checks carry most of the value, and both encode a failure this session actually hit:

1. **The three-way FRB check.** Codegen binary, Rust crate and Dart package are three separate
   installs of one version; any two agreeing is not enough.
2. **JDK resolution matching Flutter's own order**, reporting *which source* the JDK came from and
   naming JDK 25 as the known-bad case rather than emitting a generic version mismatch.

And `tools/doctor-selftest.sh` deliberately breaks each pin in a scratch copy and asserts doctor
catches it. It runs inside `just verify`. A doctor that has only ever run on a healthy machine is a
green light with no bulb behind it.

## Consequences

**Good.** Toolchain drift becomes a fast, legible failure with the fix command in the error text,
instead of a Gradle or linker error forty minutes into a build. The pins cannot silently rot, since
`.fvmrc` and `versions.env` disagreeing is itself a checked failure. Dropping the NDK trap removes a
class of bug from `T-001` rather than documenting it.

**Bad.** One more file to keep in sync, and `versions.env` is now load-bearing — editing it
carelessly breaks every build at once. That is the intended trade: it is better to have one file
that is obviously important than eight places that are quietly authoritative.

**Rejected: pinning Flutter 3.44 and installing it via fvm.** It would have matched the written
contract, but the contract was describing a version that was never current during this project's
life. Pinning to the past to avoid amending a document is the wrong way round.

**Rejected: making JDK 21 the machine default.** Fixes the build, breaks whatever else on the
machine wants 25, and makes the repo's requirements invisible to `doctor`.

**Rejected: keeping the pins as prose and trusting review.** This is exactly what produced the four
wrong pins above, within one day of them being written.
