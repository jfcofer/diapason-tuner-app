---
id: T-016
title: Move the router to typed go_router routes, as AGENTS.md §7 requires
status: todo
milestone: M3
owner: unassigned
created: 2026-10-30
---

## Goal

`apps/diapason/lib/router.dart` builds plain `GoRoute`s from string constants, though its own doc
comment and `AGENTS.md` §7 say typed routes. A typo in a path or a parameter is a runtime failure
today. Typed routes (`go_router_builder`) make it a compile error, before M3 adds real screens.

## Context

Read first: `AGENTS.md` §7 and `apps/diapason/lib/router.dart`.
Check `go_router_builder`'s current docs and the version that matches the pinned `go_router`
(golden rule 3: no guessed API). Codegen runs through `just gen`; generated files follow
`.gitattributes`.

## Acceptance criteria

- [ ] Every route is a `@TypedGoRoute` class; no string path is used outside the generated code
- [ ] `go_router_builder` is pinned in the app's dev dependencies; `just gen` produces the routes
- [ ] Navigation in `feature_*` goes through typed routes without a feature importing another
- [ ] `router.dart`'s doc comment matches what it does
- [ ] `just verify` passes, and all six CI jobs are green

## Out of scope

New screens or deep links (M3, M6).

## Implementation notes

## Verification performed
