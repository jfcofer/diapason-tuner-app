# 2026-10-04 — licence-and-protection

**Agent:** Claude Code (Opus 5.5)  **Task:** T-005  **Milestone:** M0

## Done

- **PR #1 rebase-merged.** It brought 17 single-concern commits onto `main`, in linear history.
- **Repo settings:** rebase-only merges, and head branches deleted on merge.
- **Ruleset `24468437` on the default branch, with no bypass actors.** It requires a PR and the six
  CI checks on an up-to-date branch, plus linear history and resolved review threads. Force-push
  and deletion are blocked.
- **Licence FSL-1.1-ALv2** (`adr/0017`), with `adr/0009` moved to Accepted. The official template
  text is fetched from getsentry/fsl.software, not written from memory.

## Tried and abandoned

- **An admin bypass on the ruleset.** Agents run with the owner's credentials, so a bypass for the
  owner is a bypass for every agent.
- **MIT/Apache-2.0, GPL/AGPL, PolyForm Noncommercial and BSL.** The reasons are in `adr/0017`'s
  "Rejected".

## Surprises

- Secret scanning and push protection were already enabled, which is the default for public repos.
  Dependabot *security* updates are not enabled. The weekly version updates in
  `.github/dependabot.yml` do not cover advisories that arrive between runs. Offered to the user.

## Left for next session

- The licensor in `LICENSE.md` is the git handle `jfcofer`. The owner may want their legal name.
- Then close M0 and start `T-002a`.
