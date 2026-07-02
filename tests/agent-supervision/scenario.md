# Scenario: "calc service" — spec encoded as wipe tickets

The supervisor sets up a board named **Calc Service** and files one spec ticket in
the `todo` list. This is the entire brief handed to the subordinate agent; it must
be discoverable and actionable purely through the `wipe` CLI.

## Ticket T-1 (todo) — Implement `add`

**Title:** Implement add(a, b) in calc.py

**Body / spec:**

> Create a file `calc.py` at the repository root that defines a function
> `add(a, b)` returning the sum `a + b`.
>
> Acceptance criteria:
> - `calc.py` exists at the repo root.
> - It defines `def add(a, b)` returning `a + b`.
>
> When finished: add a comment on this ticket summarizing what you did, then move
> this ticket to the `done` list.

## Expected subordinate behavior

1. Discover the CLI: `wipe skill` and/or `wipe --help`.
2. Find assigned work: `wipe ticket list --list todo --json`.
3. Read the spec: `wipe ticket show T-1 --json`.
4. Implement `calc.py`.
5. Report: `wipe comment add T-1 --body "..."`.
6. Advance: `wipe ticket move T-1 --to done` (or `wipe ticket close T-1`).

## Supervisor acceptance checks

- `calc.py` exists and defines `add`.
- `wipe ticket show T-1 --json` reports the ticket on the `done` list.
- T-1 has at least one comment authored by the subordinate.
