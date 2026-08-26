# 0002 — AGENTS.md as canonical contract, with tiered memory

**Status:** Accepted · 2026-08-25

## Context

This project is built primarily by AI agents, possibly different ones across sessions. Agent working
memory dies at the end of a session, so project memory has to live in the repository. Two failure
modes: nothing written down (every session rediscovers the codebase), or everything written down
(the agent burns its context window on stale, contradictory prose).

Tooling reality as of August 2026: `AGENTS.md` is a cross-vendor convention stewarded by the Linux
Foundation and read natively by Codex, Cursor, Copilot, Gemini CLI, Aider, Windsurf and others.
Claude Code reads `CLAUDE.md` and does **not** read `AGENTS.md` natively; Anthropic's documented
workarounds are an `@AGENTS.md` import or a symlink.

## Decision

`AGENTS.md` is the single canonical contract. `CLAUDE.md` is one `@AGENTS.md` import plus
Claude-specific mechanics. Memory is tiered with enforced budgets — contract, state, tasks,
reference, decisions, history — as specified in `docs/agents/CONTEXT_SYSTEM.md`.

The import is preferred over a symlink because it survives Windows checkouts and lets `CLAUDE.md`
carry genuinely Claude-specific content (subagents, slash commands, permissions) that would be noise
for other tools.

## Consequences

**Good.** Switching agents costs nothing. One source of truth, so no drift between tool-specific
files. Budgets enforced in CI keep the always-loaded context small, which is the difference between
an agent that starts working immediately and one that spends its first thousand tokens reading.

**Bad.** Discipline is required at session end, every session; it is wrapped in `just session-end`
precisely because relying on memory would fail. Budgets will occasionally feel arbitrary and
restrictive — that is the mechanism working.

**Revisit if** Claude Code ships native `AGENTS.md` support, at which point `CLAUDE.md` shrinks to
Claude-specific mechanics only.
