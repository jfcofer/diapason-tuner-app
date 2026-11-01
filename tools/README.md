# tools/

Scripts called by the `justfile`. Bash, POSIX-ish, `set -euo pipefail`, or `set -uo pipefail` where a
script reports every failure before exiting. No dependency the `doctor` script does not check for. Every one of them must be runnable by a human, an agent, and CI
identically — that is the whole reason they live here instead of inline in a workflow.

| Script | Job |
|---|---|
| `doctor.sh` | Verify every pinned version in `docs/DEVELOPMENT.md` §1. Must fail loudly on an FRB codegen/runtime mismatch — that one mismatch causes more lost hours than anything else in this stack |
| `check-deps.sh` | Parse `pubspec.yaml` and `Cargo.toml` files; fail on any upward or sideways dependency (`AGENTS.md` §5) |
| `docs-check.sh` | Resolve relative links and backticked paths, enforce line budgets, and hold task files, the ADR index and the lifecycle matrix to what they claim |
| `pr-check.sh` | Hold a PR's diff to the session protocol (`AGENTS.md` §2): STATE, task file, journal. CI runs it; `session-end` shares its rules |
| `check-android-release.sh` | Assert targetSdk 36, verify 16 KB page alignment on every shipped `.so`, that the engine links AAudio and no C++ runtime, and the size budgets |
| `goldens.sh` | Run goldens; refuses to update them until M3 decides how (`docs/adr/0016`) |
| `bench-compare.sh` | Compare criterion output against `rust/crates/dsp/benches/baseline.json`; fail on >10 % regression |
| `session-start.sh` | Print the reading order, branches, open PRs, open tasks and `doctor`, for the agent to reconcile with STATE.md |
| `branch-state.sh` | Sourced by the two session scripts: a branch's commits not in main, and whether it was never pushed |
| `session-end.sh` | Verify STATE.md, the active task file and a journal entry were all updated this session |
| `format-changed.sh` | Format only changed files. Called by the Claude Code post-edit hook |
| `rename.sh` | Rename the project: packages, crates, bundle IDs, display names |
| `test-dart.sh` | Melos-scoped Dart tests, or all packages when unscoped |
| `doctor-selftest.sh` | Break each pin in a scratch copy and prove `doctor.sh` catches it |
| `check-ios-release.sh` | Assert on a built `.app`: privacy manifest, mic string, MinimumOSVersion, Rust linked. Bash 3.2/BSD only — it runs on macOS |
| `ios/configure_project.rb` | Generate the iOS flavour configs, schemes, xcconfigs and Podfile (`just ios-project`). Ruby + the pinned `xcodeproj` gem |
| `jvm.sh` | Sourced, not run: what `doctor.sh` and `ktfmt.sh` must agree on (the JDK Flutter builds with, the ktfmt jar path, SHA-256) |
| `patrol.sh` | Run the pinned patrol_cli with its traps handled: no shell-profile edits, the device always named, no stdin (`adr/0026`) |
| `ktfmt.sh` | Install the pinned ktfmt jar (checksum-verified), and format or check our Kotlin (`adr/0023`) |
| `test-android-device.sh` | Run `audio_io`'s device tests on the attached phone, over adb |
| `test-integration-android.sh` | Run the patrol device tests on the attached phone (`docs/TESTING.md` §1) |
| `mic-clients.sh` | What Android reports about the app's microphone: open capture clients (exit 3 if any) and timestamped recording events, from two `dumpsys` formats measured on the Redmi (`T-002d`) |
