@AGENTS.md

# Claude Code — repo-specific notes

`AGENTS.md` above is the canonical contract and is shared with every other agent. This file only
adds Claude-specific mechanics. Keep it short; if a rule matters to all agents, it belongs in
`AGENTS.md` instead.

## Working style here

- **Plan mode** is how you meet `AGENTS.md` §3.1's plan approval.
- Use the `/session-start` and `/session-end` skills in `.agents/skills/` (each linked from
  `.claude/skills/`) — they encode the protocol in `AGENTS.md` §2 so it does not depend on you
  remembering it.
- Delegate to subagents in `.claude/agents/` when the work splits cleanly: `dsp-engineer` for
  `rust/crates/dsp`, `flutter-ui` for `core_ui`/`feature_*`, `reviewer` before you hand back.
  Subagents keep their exploration out of the main context window — use them for search-heavy
  spelunking, not for decisions.

## Context hygiene

- `/context` before large refactors; `/compact` at natural boundaries, never mid-edit.
- Do not read `docs/agents/journal/` in bulk. Grep it for a specific question.
- Generated files (`*.frb_generated.dart`, `*.g.dart`, `*.freezed.dart`) are noise — do not read
  them unless debugging codegen itself.

## Permissions

`.claude/settings.json` is the only statement of what is pre-approved, asked or denied; read it,
do not rely on a summary. Its rules match command prefixes, so `AGENTS.md` §8's "in any spelling"
is what binds. Do not propose widening the signing boundary to move faster.

One recorded exception, approved by the owner on 2026-10-09 (`T-007`): `just session-start` runs
two **read-only network** calls, `git fetch --prune` and `gh pr list`, without a prompt, even though
`gh` on its own still asks. They are how a session sees merges that happened while no agent was
running. They are bounded and never fatal. Anything that *writes* to GitHub still asks.
