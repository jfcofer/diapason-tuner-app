# Tasks

One file per unit of work. A task is a **contract written before implementation**: its acceptance
criteria are fixed while the code does not exist yet, which is what makes "done" checkable rather
than arguable.

- IDs are sequential and permanent: `T-001`, `T-002`, … Never reuse one.
- File name: `T-###-kebab-summary.md`. Status lives in the front matter, not the file name.
- One task is active at a time; `STATE.md` names it. Parallel work needs parallel branches.
- Budget: 150 lines. If a task needs more, it is two tasks.
- Statuses: `todo` → `in-progress` → `blocked` → `done` (or `abandoned`, with a reason).
- A task that turned out to be wrong is closed as `abandoned` with an explanation. It is not deleted
  — the next agent needs to know it was considered.

Copy `TEMPLATE.md` to start one.
