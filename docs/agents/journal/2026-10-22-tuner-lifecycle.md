# 2026-10-22 — tuner lifecycle

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002d (part 1)  **Milestone:** M1

## Done
- **Reconciled.** No contradictions. `git fetch` timed out over SSH; `gh` showed no new PRs.
- **Part 1, Dart only, no FFI change.**
  - A pure `engineTarget` policy decides the stream: stopped, output only, or duplex.
  - `AppLifecycle` + `FakeAppLifecycle` in `core_platform`.
  - `TunerController` owns what the user asked for and re-reads the permission on return.
  - Visibility comes from `TickerMode`.
  - Start and Stop listening controls.
  - `diapasonRoot()` is shared by bootstrap and patrol.
  - Patrol lifecycle tests, named by matrix ID.
- **The lifecycle matrix has IDs (L1 to L11), and `docs-check` checks every row's proof.** A
  `patrol` cell must carry a dated run.
- **Plan corrections, checked against the live code:**
  - `TickerMode.of` is deprecated in 3.47.1.
  - The session replays a tone after `stop()`, so the controller tracks the tone itself and stops
    it when the tuner is left. The task's "keep the tone" exception was dropped.
  - A granted permission showed no button at all.
  - Android reports "never asked" as `denied`.

## Tried and abandoned
- **`ModalRoute.isCurrentOf` for visibility**, as the approved plan said. The reviewer found it is
  false under every popup route, so a dialog, a menu, or the compact layout's bottom sheet would
  have stopped the tuner. `TickerMode` alone is right: an opaque route or an offstage branch
  stops it, a popup does not.
- **Bare `patrol` in the matrix.** CI runs no device, so a test name proved nothing. A row now
  needs the device, OS and date of a passing run.
- **Writing `just mic-clients` without a device.** It would mean guessing the `dumpsys` format.
  Deferred until the Redmi is attached.

## Surprises
- **`PRODUCT_SPEC.md` §1 and §2 want the app to open already listening.** T-002d's criterion and
  L11 say returning never listens unasked. This is recorded as an open question for M3 and was
  not decided here.
- **The analyzer wants the fake's mutable permission as a plain field** (`current`), not a method
  or a setter.

## Left for next session
- **Attach the Redmi.**
  - Measure `dumpsys`, then add `just mic-clients`.
  - Run `just test-integration-android`.
  - Do the manual rows (L3–L8).
  - Flip the verified cells in §5.
  - Push and open the PR.
- **Watch for recents:** if the Redmi reports recents as `inactive`, the L1 recents test fails by
  design.
