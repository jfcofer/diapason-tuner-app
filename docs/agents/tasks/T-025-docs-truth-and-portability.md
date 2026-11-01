---
id: T-025
title: Make the docs checkably true, and the protocol readable by every agent
status: done
milestone: M2
owner: claude
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

**Skills.** Agent Skills (`agentskills.io`) defines a `SKILL.md` folder format. Secondary sources disagree
on clients' project paths, so the first step was to check each client's own docs.

## Acceptance criteria

- [x] `just verify` passes. CI's six jobs are this PR's own gate to merge.
- [x] Every contradiction above is fixed. Where the code is wrong, the doc says so and the task is named.
- [x] `docs-check` fails on a backticked repo path that does not exist, in living docs only, with a short
      explicit list for files still planned. A self-test proves it fails on a planted path.
- [x] `STATE.md` is at about 80 lines: traps move to `DEVELOPMENT.md`, and the decisions list goes (the ADR
      index owns it).
- [x] `AGENTS.md` gains, with **no net line growth**, the rules that bind every agent: plan approval for
      engine and FFI work, checkpointing into the task file, destructive-command spellings, the signing
      boundary, and that `session-start` does not implement. Explanatory asides pay for them.
- [x] The four `.claude/commands/` become `.agents/skills/<name>/SKILL.md`, with `.claude/skills` a symlink,
      if the first step shows that this reaches other clients. If not, they stay, and this file says why.
- [x] `CONTEXT_SYSTEM.md` §5 gains three rules: a drift fix lands in the PR that finds it; a new process rule
      needs a named incident and a mechanical check; bookkeeping travels in the PR it describes.
- [x] `CONTEXT_SYSTEM.md` §6 records what was considered and not added: `llms.txt`, MCP config, `.cursor`
      rules, new memory files.

## Out of scope

`T-016`, which fixes the routes themselves. New process rules beyond these three.

## Implementation notes

**Skills: one link per skill, not a linked directory.** The plan said `.claude/skills` would be a symlink.
Claude Code's docs say a `<skill-name>` entry can be a symlink, and say nothing of the directory, so each
skill is linked. The clients page on `agentskills.io` rendered empty, so these come from each tool's docs,
2026-11-01: Codex `.agents/skills` (follows symlinks); Gemini CLI `.gemini/skills`, with `.agents/skills` as
an alias that wins; Copilot `.github/skills`, `.claude/skills` or `.agents/skills`; Cursor `.cursor/skills` or
`.agents/skills`, plus Claude's and Codex's folders for compatibility. Copilot and Cursor can reach a skill
by both paths. **Not tried in those tools.** Claude Code resolves skills at start-up, so the new links were
not exercised in the session that made them.

**The path check also scans code for `docs/*.md`,** which the task did not ask for: `BOOTSTRAP.md` was cited
in a Dart comment, so a docs-only scan would have missed the very case it came from. It skips `pr-check.sh`,
whose self-test plants such paths, and `T-###`, which is a placeholder and not an anchor. A path counts if
git knows it, so a clean clone and a built tree agree. It recognises known roots and `UPPERCASE.md`, so a
relative `dsp/src/x.rs` still goes unchecked: a real gap, kept to avoid false alarms.

**`AGENTS.md` stayed at 149 lines,** with the promoted rules paid for by asides. `just run ios` left its table.

**`tools/README.md`'s header is corrected rather than the scripts changed:** the scripts on `set -uo` report
every failure before exiting, on purpose.

## Verification performed

2026-11-01. `just verify` passes. `shellcheck tools/*.sh` is clean. Planting `tools/planted-missing.sh` and
`adr/9998` in `docs/TESTING.md` made `docs-check` name both and exit 1; restoring it, exit 0. The in-script
proof runs on every `docs-check`. Every link resolves: `test -f .claude/skills/<name>/SKILL.md` for all four.
