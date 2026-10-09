## Task

`T-###`: link the task file. Which acceptance criteria does this PR tick?

## Gate

- [ ] `just verify` is green locally (paste the summary line)
- [ ] `STATE.md`, the task file and a journal entry are updated (`AGENTS.md` §2)

## New dependencies

One line each: what it does, why nothing we already have does it, and its licence (`adr/0009`).
Write "none" if there are none.

## Decisions

Any ADR added or superseded. Write "none" if there are none.

## Real-time path

- [ ] This PR does not touch code the audio callback runs
- [ ] It does, and that code has no allocation, lock, syscall, log or panic, and is covered by the
      zero-allocation test (`AGENTS.md` §6)
