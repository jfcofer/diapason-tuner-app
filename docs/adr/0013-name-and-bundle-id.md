# 0013 — "Diapason" / "Diapasón": locale-aware label, ASCII identifiers

**Status:** Accepted · 2026-08-26

## Context

`STATE.md` carried "Final app name and bundle ID" as an open question, with `com.example.diapason`
marked as a placeholder that cannot ship. Both had to be settled before `T-001` created the native
projects, because a bundle ID is effectively immutable once an app is published and a rename touches
every layer of the tree.

The name is a *diapasón* — Spanish for tuning fork, and the reason this project exists. It is also
a crowded name. A search of Google Play found at least three tuner apps already using it:
`com.avsoft.diapason`, `com.thegolemapps.diapasonplus`, and `diapaSon`. "Diapason" is a generic
musical term in English too (an organ stop, a range), so it is descriptive and not realistically
protectable — nobody can stop us using it, and we cannot stop anyone else.

## Decision

**Display name is locale-aware. Every identifier is ASCII.**

| Locale | Launcher / store name |
|---|---|
| `en` (and default) | `Diapason` |
| `es` | `Diapasón` |

Identifiers — bundle ID, Dart package names, Rust crate names, directory names, Gradle flavours —
are ASCII `diapason` **everywhere**, with no accent, ever.

| Flavour | Bundle ID | Launcher label (en / es) |
|---|---|---|
| `dev` | `dev.jfcofer.diapason.dev` | Diapason (Dev) / Diapasón (Dev) |
| `stg` | `dev.jfcofer.diapason.stg` | Diapason (Stg) / Diapasón (Stg) |
| `prod` | `dev.jfcofer.diapason` | Diapason / Diapasón |

On Android the label is a per-flavour, per-locale string resource
(`android/app/src/<flavour>/res/values{,-es}/strings.xml`), not a manifest literal. iOS uses
`InfoPlist.strings` in `en.lproj` and `es.lproj`.

## Consequences

**Good.** The Spanish name — the real one, the one the app is named for — reaches Spanish users
without making the English store listing harder to type or search for. Keeping identifiers ASCII
sidesteps every encoding, path and tooling edge case, and those are the strings that are painful to
change later; the label, which is the part that is cheap to change, is the only place the accent
appears. The `dev.` prefix on the bundle ID is a real reverse-domain namespace under a TLD we
control, so it is unambiguously ours in a way `com.example` never was.

**Bad, and accepted knowingly.** We are the fourth "Diapason" tuner on Google Play. Store search
discoverability will be poor, and a differentiated name would be better *for growth* — this decision
trades that away for a name that means something. The doubled "dev" in `dev.jfcofer.diapason.dev` is
ugly; it appears only in the ID, never in the UI, and the alternative was renaming the flavours away
from what `docs/CI_RELEASE.md` §2 already specifies.

**If the name is ever changed**, `just rename <name> <bundle_id>` exists for exactly this and is
verified by a round-trip test. Changing the *bundle ID* after the first store release is not a
rename, it is a new app — this is the decision that is genuinely one-way.

**Rejected: "Diapasón" everywhere, accent included.** Most distinct from the existing ASCII
listings and strongest identity, but it puts a character most users cannot type into the name of a
thing they find by typing.

**Rejected: "Diapason" everywhere, no accent.** Technically the simplest, and it discards the
reason the app has this name at all for a benefit the locale-aware option already provides.
