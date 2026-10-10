---
id: T-011
title: Gate Kotlin formatting with a pinned ktfmt before any real Kotlin lands
status: done
milestone: M1
owner: claude
created: 2026-10-09
---

## Goal

Make Kotlin formatted and checked by `just verify` exactly as Dart and Rust are. `T-002b` 2b-i adds
the first Kotlin we write, and the owner decided on 2026-10-09 that the gate comes first, as its
own task with its own pin and ADR (`AGENTS.md` §8).

## Context

Read: `tools/doctor.sh` and `tools/doctor-selftest.sh` (how a pin is checked and proven),
`.github/actions/toolchain/action.yml` (how CI installs a pin), `docs/DEVELOPMENT.md` §1–2.

ktfmt facts, checked on Maven Central on 2026-10-09: the latest release is 0.64 (2026-06-24). The
artifact is `ktfmt-0.64-with-dependencies.jar` (71 MB), with a `.sha256` beside it. Its CLI has
`--kotlinlang-style`, `--dry-run` and `--set-exit-if-changed`.

## Acceptance criteria

- [x] `adr/0023` records the tool, style, scope and rejected alternatives; indexed
- [x] `KTFMT_VERSION` and `KTFMT_SHA256` pinned in `tools/versions.env`
- [x] `just install-ktfmt` (and `just setup`) installs the jar, refusing one that fails its checksum
- [x] `just fmt-check-kotlin` is part of `fmt-check`, so `verify` runs it; `just fix` formats
- [x] A misformatted `MainActivity.kt` fails `fmt-check-kotlin`; `fix` repairs it
- [x] `doctor repo-tools` checks the jar, and `doctor-selftest` proves both a wrong version and a
      wrong checksum fail (15/15)
- [x] CI's `checks` job installs the jar with the pinned JDK and runs the check; `actionlint` passes
- [x] lefthook and the agent edit hook format `.kt`, never generated `*.g.kt`
- [x] `just verify` green locally and all six CI jobs green on the PR (#15, merged 2026-10-09)

## Out of scope

Formatting Gradle `*.kts` (Flutter's templates). Kotlin lint rules. Caching the jar in CI.

## Implementation notes

- **`tools/jvm.sh`** holds what `doctor` and `ktfmt.sh` must agree on: the JDK lookup `doctor`
  already had, the jar path and the SHA-256 helper. `find_jdk` prints its result rather than
  setting globals, so ShellCheck needs no suppression.
  On this host, the `java` on PATH is JDK 25, which runs ktfmt but prints `sun.misc.Unsafe`
  deprecation warnings. The pinned JDK 21 runs it cleanly.
- **The jar lives in `${XDG_CACHE_HOME:-~/.cache}/diapason/`**, named by version. A download goes
  to a unique temp file beside it, and is moved into place only once its SHA-256 matches the pin.
  Every run re-checks the checksum (about 0.2 s).
- **`check` tells three failures apart:** unformatted (exit 1, files listed), unparseable (exit 1,
  ktfmt's error), and cannot run (exit 2: no jar, bad jar, no JDK). Each gets its own fix.
- **Review fixes:** `setup` installs the jar before `doctor`, which would otherwise stop it; `fix`
  runs ktfmt last; the self-test also finds JDK 25 on GitHub's runners (`JAVA_HOME_25_X64`).
- **The CI `checks` job now also runs `doctor java`**, because ktfmt runs on that JDK.

## Verification performed

- Download verified against Maven Central's published SHA-256. `--help` checked on 0.64.
- `fmt-check-kotlin` on today's `MainActivity.kt`: passes. On a deliberately misformatted copy:
  fails and names the file. `ktfmt.sh format` repairs it, and the check passes again. The original
  was restored from a backup.
- `just doctor-selftest`: 15/15 caught, including the two new ktfmt cases. The control run is green.
- `actionlint` and `shellcheck tools/*.sh`: clean. `lefthook validate`: "All good".
- **After the review:**
  - each `check` failure gives its own message and fix: no jar (2), a tampered jar (2), a parse
    error (1, ktfmt's error) and unformatted code (1, the file listed);
  - `format` with a path relative to `apps/diapason/` works;
  - `install` into a scratch cache holding a junk jar re-downloads, verifies and replaces it,
    leaving no temp file. A first version of the trap tripped `set -u` on a function-local path;
    fixed.
