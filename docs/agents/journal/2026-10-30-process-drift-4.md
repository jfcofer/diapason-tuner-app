# 2026-10-30 — process-drift-4

**Agent:** Claude Code (Opus 5.5)  **Task:** T-015  **Milestone:** M2

## Done
- Planned with the owner after reconciling for `T-003c`: four kinds of drift no check caught.
- `just session-end` is now step 5 of AGENTS.md §2 and step 7 of `/session-end`.
- `docs-check`: stale code names in living docs; ADR status lines need a date; blocked tasks name
  what unblocks them. Each was shown failing on a planted case.
- Gemini CLI and Aider pointers to `AGENTS.md`, from their current docs.
- Owners for every known gap: `T-016`–`T-018`, two ROADMAP lines, one `T-002f` criterion.

## Tried and abandoned
- An allowlist for platform constants in the code-name check: scoping out `PLATFORM_AUDIO.md` is
  one rule instead of a list that grows.

## Surprises
- `CONTEXT_SYSTEM.md` said Gemini CLI and Aider read `AGENTS.md` natively; neither does by default.
- `main`'s STATE.md is the 2026-10-23 snapshot; `T-003c`'s branch carries the later ones. Rebasing
  this branch after `T-003c` merges will conflict there: keep `T-003c`'s and re-apply the gap owners.

## Left for next session
- Push both branches and open their PRs (this shell has no SSH agent).
