---
description: Load project context and pick up the active task
---

Start a working session on this repository.

1. Read `docs/agents/STATE.md` in full.
2. Read the active task file it names, in full.
3. Read only the reference docs that task points to. Do not read the rest of `docs/`.
4. Run `just doctor` and report anything that is not green.
5. Run `git status` and `git log --oneline -10` to see where the working tree actually is —
   the previous session may have ended mid-change, and STATE.md may not know it.

Then give me, in under fifteen lines:

- Where the project is and what the active task is.
- The concrete next action you propose, and roughly how you would do it.
- Any contradiction you found between STATE.md, the task file, and the actual repository. This is
  the most valuable thing you can report — a stale STATE.md silently misleads every later session.
- Any open question in STATE.md that blocks the next action.

Do not start implementing. Wait for me.
