---
id: T-025
title: Make the docs checkably true, and the protocol readable by every agent
status: todo
milestone: M2
owner: unassigned
created: 2026-11-01
---

## Goal

`docs-check` passes while the docs contradict the code, because it resolves Markdown links but not
backticked paths. `STATE.md` has no headroom. Rules that bind every agent sit in Claude-only files. Fix
these once and mechanically, so `process-drift-N` tasks stop. A study of repository context files (Gloaguen
et al. 2026, arXiv 2602.11988; Python only) found they tended to lower task success and raise cost, and
advises minimal requirements. So nothing here may grow what every agent must load.

## Context

Read: `docs/agents/CONTEXT_SYSTEM.md`, `tools/docs-check.sh`, `tools/README.md`, `CLAUDE.md`,
`.claude/commands/`, `.claude/settings.json`.

**Confirmed contradictions (2026-11-01):**
- `CI_RELEASE.md`: min SDK 26 (it is 28), `com.example.diapason` (it is `dev.jfcofer.diapason`), the miri
  row's "no `unsafe` exists" (`audio_io/src/android.rs` has it).
- `AGENTS.md` §4: `just gen` runs l10n (commented out). The `verify` comment: "Everything CI checks" (it
  omits goldens, builds, `pr-check`, `check-android-release`).
- `tools/README.md`: cites `rust/benches/baselines/` (it is `rust/crates/dsp/benches/baseline.json`),
  claims `set -euo` for all, omits four scripts. `DEVELOPMENT.md`: "No cargo-ndk" (`test-android-device.sh`
  uses it).
- `STATE.md`'s "rows L1–L11 checked". `DESIGN_SYSTEM.md`'s "refine in T-004". `theme.dart` cites a
  `BOOTSTRAP.md` that does not exist. `router.dart`'s comment claims typed routes.

**Skills.** Agent Skills (`agentskills.io`) defines a `SKILL.md` folder format. Claude Code documents
`.claude/skills/`, and does not mention `.agents/`. Secondary sources disagree on other clients' project
paths. **First step: check each client's path on `agentskills.io/clients.md`.**

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] Every contradiction above is fixed. Where the code is wrong, the doc says so and the task is named.
- [ ] `docs-check` fails on a backticked repo path that does not exist, in living docs only, with a short
      explicit list for files still planned. A self-test proves it fails on a planted path.
- [ ] `STATE.md` is at about 80 lines: traps move to `DEVELOPMENT.md`, and the decisions list goes (the ADR
      index owns it).
- [ ] `AGENTS.md` gains, with **no net line growth**, the rules that bind every agent: plan approval for
      engine and FFI work, checkpointing into the task file, destructive-command spellings, the signing
      boundary, and that `session-start` does not implement. Explanatory asides pay for them.
- [ ] The four `.claude/commands/` become `.agents/skills/<name>/SKILL.md`, with `.claude/skills` a symlink,
      if the first step shows that this reaches other clients. If not, they stay, and this file says why.
- [ ] `CONTEXT_SYSTEM.md` §5 gains three rules: a drift fix lands in the PR that finds it; a new process rule
      needs a named incident and a mechanical check; bookkeeping travels in the PR it describes.
- [ ] `CONTEXT_SYSTEM.md` §6 records what was considered and not added: `llms.txt`, MCP config, `.cursor`
      rules, new memory files.

## Out of scope

`T-016`, which fixes the routes themselves. New process rules beyond these three.

## Implementation notes

## Verification performed
