# 2026-08-25 — bootstrap

**Agent:** Claude (chat, pre-Claude-Code)  **Task:** —  **Milestone:** M0

## Done
- Created the repository as documentation-first: architecture, product spec, audio engine design,
  platform integration notes, design direction, testing strategy, CI/release plan, roadmap.
- Established the agent context system (`docs/agents/CONTEXT_SYSTEM.md`) and the tiered memory
  model, with `AGENTS.md` as the canonical cross-vendor contract and `CLAUDE.md` importing it.
- Recorded ten decisions as ADRs so the first implementation session does not re-litigate them.
- Wrote T-001 (scaffold) and T-002 (audio spine) as the first two contracts.

## Tried and abandoned
- Considered having Dart own audio capture via a plugin and calling Rust only for analysis.
  Rejected: it puts the GC and the platform-channel hop inside the latency path and makes
  sample-accurate metronome scheduling impossible. → `adr/0001`.
- Considered a single `context.md` for agent memory. Rejected as the exact failure the tiered model
  exists to prevent → `adr/0002`.

## Surprises
- Verified against current sources rather than memory: Flutter 3.44 / Dart 3.12 is stable;
  flutter_rust_bridge is on 2.12.x; `AGENTS.md` is now Linux-Foundation-stewarded and read natively
  by most agents **except** Claude Code, which still reads `CLAUDE.md` — hence the `@AGENTS.md`
  import rather than a duplicated file.
- Google Play requires targetSdk 36 for new apps and updates from 31 August 2026, which is days
  away. Baked into `PLATFORM_AUDIO.md` and the release checklist rather than discovered at
  submission.

## Left for next session
- Everything. Start at `docs/BOOTSTRAP.md`, then `T-001`.
