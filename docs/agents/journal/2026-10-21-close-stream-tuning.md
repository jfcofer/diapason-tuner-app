# 2026-10-21 — close-stream-tuning

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2b-ii, T-012  **Milestone:** M1

Started 2026-10-20, ten days after the last session; ran past midnight.

## Done
- **Reconciled STATE against the repo.** The branch was unpushed; STATE's header was dated a day
  before its own rewrite; it cited `T-003-pitch-core`, which has no file; Dependabot #20 bumped
  `permission_handler` to 13 against the hold.
- **Dependabot** (owner-approved): rebase-merged #21 (actions), #22 (libc, proptest), #25
  (build_runner). Closed #20. Dependabot itself closed #23 when asked to rebase and reopened only
  build_runner as #25; go_router and pigeon will come back on a later run.
- **T-012** (#24): each of those three drifts is now enforced in code — a Dependabot ignore,
  a header-date check in `session-end`, and a task-slug existence check in `docs-check`. Each check
  was shown to fail on a planted failure, then pass when it was reverted.
- **2b-ii closed without backlog shedding** (`adr/0025`, rewritten before merge), with the owner's
  approval. Buffer growth, the preset fallback, `callbackLoad` and the `.so` gate stay.
- **Part 3 split** into slices of `T-002`: `T-002d` lifecycle, `T-002e` overlay and first
  golden, `T-002f` latency. T-002b had reached its 150-line budget; it is condensed and now points
  to them.

## Tried and abandoned
- **Shedding from the counter gap's floor** (`backlog.rs`). The rerun on the Redmi failed
  `the_microphone_keeps_up_once_warmed_up`: 51,792 frames shed in 2 s with nothing stalling;
  ~170,000 frames shed in 5 s of the report; input underruns ~70/s. **The decisive control:**
  `main`'s code, run in a worktree that same hour on the same paths (output not FAST, input
  low-latency), had 0 underruns after warm-up and passed 5/5. With shedding cut: 6/6 in debug and
  release, again 0 after warm-up.
- **The floor's premise was wrong.** Sampled from the test thread, the raw gap stayed within
  0–960 frames (one burst). There was no ~1,700-frame constant offset; that figure in the old
  `backlog.rs` test was inferred, not measured. The reviewer had also found the floor absorbs any
  rise under its threshold, so latency could build up without ever being shed.

## Surprises
- **The first `just session-start` failed in `doctor`.** It passed on an immediate rerun and could
  not be reproduced. The failing line was not captured, so the cause is unknown.
- **The output was refused the FAST path in today's runs on `main` too** (low_latency false), so
  that is not caused by 2b-ii. `T-002f` owns finding why; part 1 had it granted.
- **`apps/diapason/lib/router.dart` uses plain `GoRoute`s**, whereas `AGENTS.md` §7 says typed
  routes. Recorded in STATE's known gaps; not fixed here.
- **The `doctor` selftest has no counterpart for `docs-check` or `session-end`**, so their checks
  are proven by hand (T-012, out of scope).

## Left for next session
- The owner merged #24 (T-012). 2b-ii was rebased onto it (STATE.md conflicted; this session's
  rewrite already carried T-012's two edits) and opened as #27. The owner merges it.
- The profile-app check of `callbackLoad` on the Redmi, if the owner wants it before merge.
- Then `T-002d` (lifecycle) in plan mode: the microphone still stays open once the tuner listens.
