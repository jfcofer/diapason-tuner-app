# 2026-10-09 — process-drift

**Agent:** Claude Code (Opus 5.5)  **Task:** T-009, lands T-002b part 1, plans part 2a
**Milestone:** M1

## Done
- Rebased T-002b part 1 onto `main`. Re-verified it (`just verify`, and on the Redmi: 5/5 and the
  canary). Opened #11, which the owner merged. Closed T-008, which #9 had merged.
- **T-009:**
  - `session-start` now lists every open task with its PR state;
  - `adr/0021` makes the docs' panic claim true;
  - stale `rust/crates/ffi` paths fixed, and the README's "Oboe".
- Planned T-002b part 2a with the owner, who split part 2 into 2a and 2b. The plan is summarised in
  STATE.md and in the task's notes.

## Tried and abandoned
- **Detecting stale tasks from branches with `git cherry`.** By the time it ran, the owner had
  merged #11 and the branch was gone, both locally and on GitHub. Merged branches get deleted, so
  PR head names are the signal that survives.
- **Panic denials in `diapason_ffi`'s `[lints]` table.** FRB's generated decoder in the same crate
  unwraps. The denials are scoped to `pub mod api` instead.

## Surprises
- **`ARCHITECTURE.md` §7 and `AGENTS.md` §6 promised `catch_unwind` at the FFI.** But
  `panic = "abort"` in release, and the AAudio callbacks are `extern "C"`, which abort on panic
  since Rust 1.81. Fixed by `adr/0021`.
- **`gh pr list --state merged` orders by creation, not merge time.** It listed #7 above #6. Now
  sorted by `mergedAt`.
- **There was no home for a stream supervisor.** `engine`'s `clippy.toml` denies `Mutex::lock` and
  `Instant::now` crate-wide, `ffi` must contain no logic, and `audio_io` is platform code. Part 2a
  adds a `session` crate (ADR 0022).
- **`engineHandleProvider` lives in `feature_tuner`.** The metronome cannot reach it there, so 2a
  moves it down to `audio_engine`.

## Left for next session
- Land T-009 (owner).
- Then T-002b part 2a: write ADR 0022, then the `session` crate.
- Verify on the Redmi that revoking the microphone permission kills the process. Android does this,
  and if so it decides how "revocation while running" is tested.
