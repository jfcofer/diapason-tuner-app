---
id: T-017
title: Give audio_engine Swift Package Manager support before Flutter makes it an error
status: todo
milestone: M3
owner: unassigned
created: 2026-10-30
---

## Goal

Flutter 3.47 warns that `packages/audio_engine` (a CocoaPods-only plugin, built by cargokit) lacks
Swift Package Manager support, and that this will become an error. iOS is CI-only for this project
(no Mac), so the change must be proven on CI's macOS job before a Flutter bump forces it.

## Context

Read first: `docs/CI_RELEASE.md` §1 (the `build-ios` job); `docs/adr/0003` (frb) and the cargokit
notes in `packages/audio_engine`; Flutter's current docs on SwiftPM for plugin authors (check the
version pinned in `tools/versions.env`). Trap: cargokit is not our code (`STATE.md`).

## Acceptance criteria

- [ ] `audio_engine` builds for iOS through SwiftPM, with CocoaPods still working until removed
- [ ] The Rust static library is linked into the shipped iOS app (verify on the binary, not the build)
- [ ] CI's `build-ios` job passes; the SwiftPM warning is gone from its log
- [ ] Any decision that closes off CocoaPods or cargokit gets an ADR
- [ ] `just verify` passes, and all six CI jobs are green

## Out of scope

Removing CocoaPods from the app; a Flutter version bump (its own task and ADR).

## Implementation notes

## Verification performed
