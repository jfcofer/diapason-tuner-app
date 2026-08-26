@AGENTS.md

# Claude Code — repo-specific notes

`AGENTS.md` above is the canonical contract and is shared with every other agent. This file only
adds Claude-specific mechanics. Keep it short; if a rule matters to all agents, it belongs in
`AGENTS.md` instead.

## Working style here

- **Plan mode first** for anything touching the Rust engine, the FFI surface, or more than three
  files. Present the plan, get approval, then implement.
- Use the `/session-start` and `/session-end` commands in `.claude/commands/` — they encode the
  protocol in `AGENTS.md` §2 so it does not depend on you remembering it.
- Delegate to subagents in `.claude/agents/` when the work splits cleanly: `dsp-engineer` for
  `rust/crates/dsp`, `flutter-ui` for `core_ui`/`feature_*`, `reviewer` before you hand back.
  Subagents keep their exploration out of the main context window — use them for search-heavy
  spelunking, not for decisions.
- Long tasks: checkpoint into the task file as you go. Assume the session can end at any moment and
  another agent (possibly not Claude) picks it up with only `STATE.md` and the task file.

## Context hygiene

- `/context` before large refactors; `/compact` at natural boundaries, never mid-edit.
- Do not read `docs/agents/journal/` in bulk. Grep it for a specific question.
- Generated files (`*.frb_generated.dart`, `*.g.dart`, `*.freezed.dart`) are noise — do not read
  them unless debugging codegen itself.

## Permissions

`.claude/settings.json` pre-approves the safe loop (`just *`, `cargo *`, `flutter test`, `dart *`,
read-only git). Anything destructive — `git push`, `rm -rf`, credential access, editing
`android/app/build.gradle.kts` signing blocks — will ask. That boundary is deliberate; do not
propose widening it to move faster.
