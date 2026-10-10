# 0023 — Format Kotlin with a pinned ktfmt, kotlinlang style

**Status:** Accepted · 2026-10-09

## Context

Until now the repo's Kotlin was Flutter's five-line `MainActivity`, and nothing formatted or
checked it. `T-002b` part 2b adds the first Kotlin we write ourselves: the capabilities channel in
`core_platform`, plus Pigeon's generated host API. Every other language here is formatted by a
pinned tool and checked in `just verify` (`dart format`, `cargo fmt`). The owner decided on
2026-10-09 that Kotlin gets the same gate **before** any real Kotlin lands, as its own task.

The candidates were ktfmt, ktlint and Spotless.

## Decision

- **ktfmt 0.64**, `--kotlinlang-style`: ktfmt's take on the Kotlin coding conventions, a 4-space
  indent and 100 columns (checked: a 101-column line is rewrapped), matching `.editorconfig`.
  ktfmt is a formatter with no configuration, like `dart format` and `rustfmt`, so formatting is
  never a review topic.
- **The pin is a version and a SHA-256** in `tools/versions.env`: the `with-dependencies` jar
  from Maven Central, verified against the checksum Maven Central publishes beside it.
- **One install path, `tools/ktfmt.sh install`**, used by `just setup` (before `doctor`, which
  fails without the jar) and by CI's toolchain action. The jar goes to the user cache, named by
  version, so a changed pin never runs a stale jar, and worktrees share it. A download that fails
  its checksum never replaces the jar, and **every run re-checks the jar** against the pin.
- **It runs on the JDK Flutter builds with.** `tools/jvm.sh` holds what `doctor` and `ktfmt.sh`
  must agree on: that JDK lookup, the jar's path, and the SHA-256 helper.
- **Scope: tracked `*.kt` we author.** Generated `*.g.kt` (Pigeon) is excluded. Gradle's `*.kts`
  stay as Flutter's templates write them, so a Flutter upgrade can diff them cleanly.
- **Gates:**
  - `just fmt-check-kotlin` is part of `fmt-check`, so `verify` runs it;
  - CI's `checks` job runs it with the pinned JDK;
  - `doctor repo-tools` checks the jar and its checksum, and `doctor-selftest` proves both fail;
  - lefthook and the agent's edit hook format `.kt` on the way in.

## Consequences

**Good.** Kotlin is gated from its first real line, the same way Dart and Rust are. A changed or
tampered jar is caught. No Gradle run is needed to format: Gradle takes tens of seconds to start,
and the formatter takes about one.

**Bad.** The jar is 71 MB, downloaded once per machine and once per CI run of `checks`. It is
fetched from Maven Central's CDN without a cache step; add one if it shows up in CI time. Like
every pin, bumping it is a task of its own.

**Rejected.**
- **ktlint:** it is a linter with formatting attached and many rule switches, which invites
  suppressions (`AGENTS.md` §3.2).
- **Spotless (Gradle):** it would tie formatting to a Gradle build of the Flutter app, which is
  slow and needs the whole Android toolchain.
- **Formatting the Gradle `*.kts`:** churn in files we do not own.
