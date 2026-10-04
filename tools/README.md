# tools/

Scripts called by the `justfile`. Bash, POSIX-ish, `set -euo pipefail`, no dependency the `doctor`
script does not check for. Every one of them must be runnable by a human, an agent, and CI
identically — that is the whole reason they live here instead of inline in a workflow.

Created in T-001:

| Script | Job |
|---|---|
| `doctor.sh` | Verify every pinned version in `docs/DEVELOPMENT.md` §1. Must fail loudly on an FRB codegen/runtime mismatch — that one mismatch causes more lost hours than anything else in this stack |
| `check-deps.sh` | Parse `pubspec.yaml` and `Cargo.toml` files; fail on any upward or sideways dependency (`AGENTS.md` §5) |
| `docs-check.sh` | Resolve every relative link in `docs/`, enforce the line budgets, verify the ADR index matches the directory |
| `check-android-release.sh` | Assert targetSdk 36, verify 16 KB page alignment on every shipped `.so`, check the AAB size budget |
| `goldens.sh` | Run goldens; refuses to update them until M3 decides how (`docs/adr/0016`) |
| `bench-compare.sh` | Compare criterion output against `rust/benches/baselines/`; fail on >10 % regression |
| `session-end.sh` | Verify STATE.md, the active task file and a journal entry were all updated this session |
| `format-changed.sh` | Format only changed files. Called by the Claude Code post-edit hook |
| `rename.sh` | Rename the project: packages, crates, bundle IDs, display names |
| `test-dart.sh` | Melos-scoped Dart tests, or all packages when unscoped |
| `doctor-selftest.sh` | Break each pin in a scratch copy and prove `doctor.sh` catches it |
| `check-ios-release.sh` | Assert on a built `.app`: privacy manifest, mic string, MinimumOSVersion, Rust linked. Bash 3.2/BSD only — it runs on macOS |
| `ios/configure_project.rb` | Generate the iOS flavour configs, schemes, xcconfigs and Podfile (`just ios-project`). Ruby + the pinned `xcodeproj` gem |
