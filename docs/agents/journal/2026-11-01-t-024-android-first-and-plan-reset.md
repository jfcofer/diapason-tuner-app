# 2026-11-01 — t-024-android-first-and-plan-reset

**Agent:** Claude Code (Opus 5.5)  **Task:** T-024  **Milestone:** M2

## Done
- Audited the live repo: the Flutter side, the Rust side and the process. The main finding is that
  `dsp::chain` has no caller outside its own tests and benches, so the app cannot show a note.
- The owner decided four things: a real note on the Redmi next; the strobe follows Peterson's asymmetric
  sweep; the tuner listens after one consent; 1.0 is Android only.
- Wrote `adr/0035`, `adr/0036` and task files `T-025`–`T-033`, `T-003d`, `T-003g`, and amended the specs.

## Tried and abandoned
- **Blocking `T-002c`,** as the plan said. `docs-check` keeps a parent open while a slice is, so `T-002`
  could never have closed. Abandoned instead, with how to restart it in the file.
- **A single ring,** and an open arc with a centred note. The owner chose the asymmetric sweep, which is the
  closest of the three to Peterson's design, so `T-033` limits what is taken and M7 carries a legal review.

## Surprises
- `session-start` printed `Permission denied (publickey)` for its fetch, and I told the owner to run
  `ssh-add`, inferring an empty ssh-agent. Minutes later plain and batch-mode fetches both worked with the
  agent still empty. The inference was wrong and the cause is unknown; `T-022` carries that.
- `T-022`'s own text would have shipped a bug: `%(upstream:track)` is empty for a branch in sync and for one
  with no upstream.
- `DESIGN_SYSTEM.md` §1 says the strobe's rate is in Hz and §3 says it follows cents. Not yet fixed: `T-032`
  chooses cents, and `T-033` rewrites the document.
- `ROADMAP.md`'s M1 still said four devices after `T-002` amended it. Fixed here.

## Left for next session
- `T-022`, then `T-025`. `T-026` needs the Redmi attached and the owner playing.
