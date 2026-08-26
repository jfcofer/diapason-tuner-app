# Documentation map

Read this when you do not know where something lives. **One fact has exactly one home.** If you
find the same rule stated in two places, delete one and link to the other.

## Tier 0 — always loaded

| File | Owns |
|---|---|
| [`/AGENTS.md`](../AGENTS.md) | The contract every agent follows: protocol, boundaries, conventions |
| [`/CLAUDE.md`](../CLAUDE.md) | Claude Code mechanics only |

## Tier 1 — current state (read every session)

| File | Owns |
|---|---|
| [`agents/STATE.md`](agents/STATE.md) | Where the project is *right now*. Rewritten each session |
| [`agents/tasks/`](agents/tasks/) | Units of work with acceptance criteria. One active at a time |

## Tier 2 — reference (read on demand, when a task points here)

| File | Owns |
|---|---|
| [`PRODUCT_SPEC.md`](PRODUCT_SPEC.md) | What the app does. Features, states, edge cases, non-goals |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Package graph, data flow, threading model, state management |
| [`AUDIO_ENGINE.md`](AUDIO_ENGINE.md) | Pitch detection, metronome scheduling, RT-safety, budgets |
| [`PLATFORM_AUDIO.md`](PLATFORM_AUDIO.md) | iOS/Android session config, permissions, background, gotchas |
| [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md) | Visual direction, tokens, motion, adaptive layout, a11y |
| [`REPO_LAYOUT.md`](REPO_LAYOUT.md) | Every directory, what belongs in it, what does not |
| [`TESTING.md`](TESTING.md) | Test taxonomy, fixtures, what each change type must prove |
| [`DEVELOPMENT.md`](DEVELOPMENT.md) | Toolchain versions, setup, `just` recipes, troubleshooting |
| [`CI_RELEASE.md`](CI_RELEASE.md) | Pipelines, flavors, signing, store submission, versioning |
| [`ROADMAP.md`](ROADMAP.md) | Milestones M0–M7 with exit criteria |
| [`BOOTSTRAP.md`](BOOTSTRAP.md) | How this repo was created and what session one does. **Delete after M0** |

## Tier 3 — decisions

| File | Owns |
|---|---|
| [`adr/`](adr/) | Architecture Decision Records. Immutable once Accepted; supersede instead |
| [`adr/README.md`](adr/README.md) | Index and the ADR process |

## Tier 4 — history (grep, never bulk-read)

| File | Owns |
|---|---|
| [`agents/journal/`](agents/journal/) | Append-only session log. Why something was done, what failed |
| [`agents/GLOSSARY.md`](agents/GLOSSARY.md) | Domain vocabulary: cents, NSDF, clarity, lock, subdivision |
| [`agents/CONTEXT_SYSTEM.md`](agents/CONTEXT_SYSTEM.md) | How this memory system works and why |

## Rules for these documents

1. **Budgets.** `AGENTS.md` ≤ 200 lines, `STATE.md` ≤ 120, a task file ≤ 150, an ADR ≤ 120.
   Reference docs have no hard cap but must be skimmable with headings.
2. **Specs describe intent, code describes behaviour.** Do not paste code into reference docs; name
   the file instead. A doc that mirrors code will drift and lie.
3. **Every doc states its owner concern in the first paragraph.** If you cannot, it should not exist.
4. `just docs-check` verifies links resolve, budgets hold, and the ADR index matches the directory.
