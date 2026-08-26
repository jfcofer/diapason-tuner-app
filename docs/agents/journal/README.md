# Journal

Append-only session log. One file per session: `YYYY-MM-DD-short-slug.md`.

**Do not read this directory in bulk** — it is grepped, not browsed. Its job is to answer "did we
already try this?" and "why on earth is it done that way?" months later, cheaply.

Never edit a past entry. If it was wrong, say so in a new one.

Template:

```markdown
# YYYY-MM-DD — <slug>

**Agent:** <tool/model>  **Task:** T-###  **Milestone:** M#

## Done
- …

## Tried and abandoned
- … and why. This section is the most valuable part of the file.

## Surprises
- Anything that contradicted a doc, a version, or an assumption. If a doc was wrong, say which,
  and confirm you fixed it.

## Left for next session
- …
```
