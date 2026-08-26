# 0006 — Pub workspaces + Melos

**Status:** Accepted · 2026-08-25

## Context

The Flutter side is a multi-package monorepo (app shell, features, core libraries, FFI plugin).
Since Dart 3.6, pub has native workspace support: one root `pubspec.yaml` lists members, dependency
resolution happens once, and the analyzer treats the repo as a single context. Melos predates this
and now builds on top of it.

## Decision

Pub workspaces for resolution and analysis; Melos 7.x on top for orchestration (running a script
across packages, change detection against a base ref). Cargo workspace for Rust, with the FFI crate
living inside the plugin package as cargokit expects.

## Consequences

**Good.** One `pub get`, one lockfile, one analyzer context — noticeably lower memory and instant
cross-package navigation, which matters when an agent is reading the repo. Melos gives change-scoped
CI: analyse and test only the packages affected by a diff.

**Bad.** All packages share one dependency resolution, so a version conflict between two packages
must be resolved rather than isolated. Two tools where teams sometimes expect one, and plenty of
outdated tutorials mixing pre-workspace Melos with the current model — follow this repo, not blog
posts.

**Rejected: a single flat package.** The boundaries in `AGENTS.md` §5 would be conventions rather
than compile errors, and conventions do not survive contact with an agent in a hurry.
