# Bootstrap — how to start

This file explains what this repository is at the moment you first open it, and exactly what the
first Claude Code session should do. **Delete it once M0 is complete** — it has no reason to exist
after that, and a stale bootstrap doc is a trap for a later agent.

## What you have

A repository containing no application code, and:

- `AGENTS.md` — the contract every agent works under. Read it first.
- `docs/` — the architecture, the product spec, the audio engine design, the platform integration
  notes, the design direction, testing, CI/release, and the roadmap.
- `docs/adr/` — ten decisions already made, with the alternatives that were rejected and why.
- `docs/agents/` — the memory system: `STATE.md`, tasks, journal, glossary.
- `justfile`, lint configs, toolchain pins, and a CI workflow — as contracts, mostly not yet
  implemented. Recipes marked `[T-001]` are what the first task builds.
- `.claude/` — settings, four slash commands, three subagents.

Everything here is a starting position, not scripture. If a decision is wrong for you, change it —
but change it in an ADR, so the next agent inherits the reasoning rather than a mystery.

## Before the first session

Two things are worth deciding first, because they are annoying to change later:

1. **The name and bundle ID.** `Diapasón` / `com.example.diapason` are placeholders.
   `just rename` will exist after T-001; doing it earlier by hand is also fine.
2. **Your four reference devices** (`docs/TESTING.md` §7). The performance budgets are meaningless
   without physical devices to measure on, and "a cheap Android phone" is the one that matters most.

## The first session

```bash
cd diapason
git init && git add -A && git commit -m "chore: bootstrap architecture and agent context system"
claude
```

Then, in Claude Code:

```
/session-start
```

It will read `STATE.md`, find `T-001-scaffold`, run `just doctor` (which will fail — nothing is
installed yet, and that is the correct first finding), and propose a plan. Review the plan properly:
this is the session that determines whether every later session is cheap or expensive.

Then let it build T-001. Expect it to take a while and to hit real friction at the Rust/Flutter
build boundary — that friction is why T-001 exists as its own task rather than being smeared across
the first five.

End with:

```
/session-end
```

## What to resist

- **Starting with the pretty screen.** The strobe ring is the most enjoyable part of this project
  and the least risky. If it gets built before the audio spine works on a real device, you will end
  up rebuilding it around whatever the engine turns out to actually be able to provide.
- **Skipping the acceptance criteria.** They were written before the code existed, which is the only
  time it is possible to write honest ones.
- **Letting `just verify` stay red.** The first time it is red and the session ends anyway, the gate
  stops meaning anything, and every later session inherits an unknown baseline.
- **Growing `AGENTS.md`.** It is loaded on every request by every agent. Things that "would be
  useful to know" belong in `docs/`; only things that change behaviour belong in the contract.

## If you are not using Claude Code

Everything here is tool-agnostic by design. `AGENTS.md` is the standard cross-vendor contract, and
`just session-start` / `just session-end` do what the slash commands do. The only Claude-specific
pieces are `.claude/` and the `@AGENTS.md` import at the top of `CLAUDE.md`.
