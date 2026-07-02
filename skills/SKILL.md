---
name: wipe
description: Drive a wipe board — a git-native, CLI-first task board for humans and agents. Use when you need to read or update tickets, lists, comments, labels, or board state in a project that has a `.wipe/` directory (or should have one). All interaction is through the `wipe` CLI with `--json`.
---

# wipe — agent operating guide

`wipe` is a git-native task board that lives inside a repository under `.wipe/`.
**As an agent you interact only through the `wipe` CLI — never read or edit files
under `.wipe/` directly.** The CLI keeps the on-disk JSON deterministic and
merge-friendly; hand-editing breaks that guarantee.

## Golden rules

1. Add `--json` to every command. Output is a single JSON object/array on stdout.
2. On success the exit code is `0`. On failure it is non-zero and, in `--json`
   mode, stdout contains `{"ok": false, "error": "..."}`. Always check the exit code.
3. Never write to `.wipe/` yourself. Use the commands below.
4. IDs are stable: tickets are `T-<n>`, comments `c-<n>`, lists are kebab-case
   slugs (e.g. `in-progress`).
5. Prefer small, explicit commands over guessing. Run `wipe <group> --help` to
   discover exact flags — the CLI is self-documenting.

## Setup

```bash
wipe init .            # create a board in the current project (once)
wipe status --json     # see lists and tickets
```

## Everyday flows

Create and place a ticket:

```bash
wipe ticket create --title "Add login" --type feature --priority high --json
# -> {"id":"T-1", ...}
```

Move a ticket across lists (lists come from `wipe list show`):

```bash
wipe ticket move T-1 --to in-progress --json
wipe ticket close T-1 --json      # convenience: move to the done list
```

Inspect a ticket:

```bash
wipe ticket show T-1 --json
wipe ticket list --list in-progress --json
```

Collaborate via comments (this is the human↔agent / agent↔agent channel):

```bash
wipe comment add T-1 --body "Spec clarified: use OAuth" --json
wipe comment list T-1 --json
```

Labels and tags:

```bash
wipe label create needs-review --color "#f5a623" --json
wipe label assign T-1 needs-review --json
wipe tag add T-1 backend --json
```

## Working with a supervisor

When another agent or a human supervises you, treat tickets as the unit of work
and comments as the conversation. Typical loop:

1. `wipe ticket list --list todo --json` to find assigned work.
2. Do the work in the repo.
3. `wipe comment add <id> --body "<what you did / questions>" --json`.
4. `wipe ticket move <id> --to in-progress|done --json` to reflect status.

Keep comments concise and factual; they are the spec-driven coordination record.

## Discoverability

Every command and group supports `--help`. If unsure about a flag, run
`wipe ticket --help`, `wipe comment --help`, etc. `wipe doctor` reports whether
you are inside a board and whether git is available.
