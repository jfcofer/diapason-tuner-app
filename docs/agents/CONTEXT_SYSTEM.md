# The context system

This document owns **how project memory works** — what gets written where, so that a session ending
(or an agent being swapped for a different one) costs almost nothing.

## 1. The problem this solves

An AI agent's working memory dies with the session. A project built by agents therefore has to keep
its memory *in the repository*, in files, in a form the next agent can load cheaply. Two failure
modes bracket the design:

- **Amnesia** — nothing was written down, so every session rediscovers the codebase, re-litigates
  settled decisions, and re-introduces bugs that were already fixed once.
- **Context rot** — everything was written down, in nine overlapping documents that now contradict
  each other, and the agent burns its window reading stale prose before it writes a line of code.

The cure for both is the same: **a small number of files with sharply separated jobs, hard size
budgets, and a protocol that says exactly when each is read and written.**

## 2. The tiers

| Tier | File | Read | Written | Budget |
|---|---|---|---|---|
| 0 · Contract | `/AGENTS.md` | Every session, automatically | Rarely, deliberately | 200 lines |
| 1 · State | `agents/STATE.md` | Every session, first thing | Every session, last thing | 120 lines |
| 1 · Work | `agents/tasks/T-###-*.md` | The active one | During the task | 150 lines |
| 2 · Reference | `docs/*.md` | On demand, when a task points there | When the design changes | — |
| 3 · Decisions | `docs/adr/*.md` | When touching the decided area | Append-only, supersede | 120 lines |
| 4 · History | `agents/journal/*.md` | **Never in bulk — grep only** | Append one per session | — |

The separation is the point. STATE is *what is true now* and is rewritten wholesale, so it never
accumulates. The journal is *what happened* and is append-only, so it never loses anything. Mixing
them produces a file that is both incomplete and too long — the most common failure in hand-rolled
agent memory setups.

## 3. Why each file exists

**`AGENTS.md` — the contract.** Loaded into every session by every tool, so every line costs tokens
on every request. It contains only rules that change behaviour: boundaries, protocol, conventions,
prohibitions. It never contains status, history, or explanation. It is deliberately the file that is
hardest to get permission to grow.

It is `AGENTS.md` rather than `CLAUDE.md` because the format is now a cross-vendor standard
stewarded by the Linux Foundation and read natively by Codex, Cursor, Copilot, Gemini CLI, Aider and
others. Claude Code reads `CLAUDE.md`, so `CLAUDE.md` is one `@AGENTS.md` import plus Claude-specific
mechanics. One source of truth; no drift between tools. (`adr/0002-agent-context-system.md`.)

**`STATE.md` — the handoff.** The answer to "I am a competent engineer who has never seen this
repo; what is going on and what do I do next?" It is a snapshot, and rewriting it in full each
session is what keeps it honest. If it is growing, something belongs in a task file or the journal.

**Task files — the unit of work.** A task is a contract with acceptance criteria written *before*
implementation. This is what makes agent work reviewable: the criteria were fixed before the code
existed, so "it works" is not a matter of opinion at the end. One task active at a time; parallel
tasks need parallel branches, not parallel edits.

**ADRs — the reasons.** Code records what was decided; an ADR records what was rejected and why. The
second is what stops an agent from "improving" a deliberate choice six weeks later. Immutable once
Accepted: to change a decision you write a new ADR that supersedes it, so the history of the
project's thinking stays readable.

**The journal — the archaeology.** Append-only, one entry per session: what was done, what was tried
and abandoned, what surprised us. Nobody reads it end to end, including agents; it is grepped when
someone asks "did we already try X?" — which is exactly the question that wastes the most time when
it cannot be answered.

## 4. The protocol

Written out in `AGENTS.md` §2 because that is the file every agent loads. The mechanics are wrapped
in `just session-start` and `just session-end` (and `.claude/commands/` for Claude Code) so that
compliance does not depend on an agent remembering — the recipe prints the reading list, shows repo
status, and on exit runs `just verify` and checks that STATE, the task file and the journal were all
touched.

## 5. Rules that keep it from rotting

1. **One fact, one home.** If a rule appears in two files, one of them is now wrong. Link instead.
2. **Budgets are enforced**, by `just docs-check`, in CI. A file over budget fails the build. This
   is unpopular and it is the only thing that actually works.
3. **Reference docs describe intent; code describes behaviour.** Do not paste code into docs. A doc
   that mirrors an implementation will drift and then lie, and a lying doc is worse than none.
4. **Delete aggressively.** A doc nobody has read in three milestones is a liability. Deleting it is
   a normal commit, and the journal remembers it existed.
5. **Never rewrite history.** ADRs are superseded; journal entries are appended; STATE is replaced.

## 6. Deliberately not used

- **Vendor-specific memory formats** (`.cursorrules`, tool-local memory files) — they fragment the
  truth across tools and die when you switch.
- **A vector database or RAG layer over the repo.** Grep and a good file layout beat it at this
  scale, are deterministic, and are reviewable in a diff.
- **Auto-generated summaries of the codebase.** They go stale silently, which is the worst failure
  mode a memory file can have.
- **A single giant `context.md`.** It is the thing every one of these rules exists to prevent.
