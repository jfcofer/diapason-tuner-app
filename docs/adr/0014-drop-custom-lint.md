# 0014 — Drop custom_lint; riverpod_lint is a first-party analyzer plugin

**Status:** Accepted · 2026-08-26

## Context

`AGENTS.md` §7 specified `custom_lint` enabled, `analysis_options.yaml` declared it under
`analyzer.plugins`, and `just lint` ran `dart run custom_lint` as a separate pass. That was correct
guidance when it was written; it stopped being correct before any code existed to lint.

`T-001` could not resolve the workspace at all. The chain, in the order pub reported it:

- `riverpod_lint` 3.1.8 needs `analyzer_plugin ^0.14.0`; `custom_lint` 0.8.1 is still on `^0.13.0`.
- Pinning back to `riverpod_lint` 3.1.3, the last release on `analyzer_plugin ^0.13`, then failed on
  `analyzer`: 3.1.3 wants `^9.0.0`, `custom_lint` 0.8.1 wants `^8.0.0`.
- There is no compatible pair. `custom_lint`'s newest release is on `analyzer ^8`; `riverpod_lint`
  3.1.8 is on `analyzer ^13`, which is the generation Dart 3.13 ships.

Inspecting `riverpod_lint` 3.1.8's dependencies explains why: it does not depend on `custom_lint`
at all any more. It depends on **`analysis_server_plugin`**, Dart's own analyzer plugin API. The
package moved to first-party plugins and `custom_lint` did not follow.

## Decision

`custom_lint` is removed from this repository. `riverpod_lint` is declared as a first-party analyzer
plugin via the **top-level** `plugins:` key in `analysis_options.yaml`:

```yaml
plugins:
  riverpod_lint: ^3.1.8
```

Note this is a *different key* from the old `analyzer.plugins:` list, which was the `custom_lint`
mechanism. Findings now surface in `dart analyze` directly, so `just lint` has one Dart pass instead
of two. `AGENTS.md` §7 is updated to match.

## Consequences

**Good.** One lint pass rather than two, and a measurably faster one — `custom_lint` ran its own
analysis server alongside the real one. The plugin API is maintained by the Dart team rather than by
a single-maintainer package, which is what caused this breakage. Lint findings appear in the IDE
through the normal analysis server with no extra setup.

**Bad.** We lose the ability to write repo-specific lints as easily as `custom_lint` allowed. Some
of the boundaries in `AGENTS.md` §5 would have been natural custom lints — a `feature_*` importing
another `feature_*`, `core_domain` importing Flutter. Those are enforced by `just check-deps`
instead, which is a script rather than an editor squiggle: it catches the same violations, later.
If enough of them accumulate to justify it, the replacement is a first-party plugin of our own on
the same `analysis_server_plugin` API, not a return to `custom_lint`.

**Rejected: pinning the whole toolchain back to what `custom_lint` supports.** That means
`analyzer ^8` and an older Dart, undoing `adr/0011` to keep one dev dependency. Backwards.

**Rejected: dropping `riverpod_lint` and keeping `custom_lint` for future custom rules.**
`riverpod_lint` catches real Riverpod misuse today; `custom_lint` would catch hypothetical rules we
have not written. Keeping the tool that finds nothing over the tool that finds something is the
wrong way round.
