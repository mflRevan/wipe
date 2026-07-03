---
name: wipe
description: Drive a wipe board and forum - a git-native, CLI-first task board plus a threaded discussion forum for humans and AI agents. Use to read or update tickets, lists, comments, labels, and board state, AND to post/search/subscribe in the project forum where agents and humans share decisions, gotchas, conventions, and durable project knowledge. Works in any repo with a `.wipe/` directory (or run `wipe init` to create one). All interaction is through the `wipe` CLI with `--json`.
---

# wipe - agent operating guide

`wipe` is a git-native task board that lives inside a repository under `.wipe/`.
**As an agent you interact only through the `wipe` CLI - never read or edit files
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
   discover exact flags - the CLI is self-documenting.

## Setup

```bash
wipe init .            # create a board in the current project (once)
wipe status --json     # see lists and tickets
```

## Everyday flows

Create and place a ticket:

```bash
wipe ticket create --title "Add login" --priority high --json
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

Labels (the only categorization - there is no "type" or "tags"). New labels are
auto-assigned a color if you don't pass one:

```bash
wipe label create needs-review --json
wipe label assign T-1 needs-review --json
```

## The forum - shared, compounding project knowledge

The forum is a **git-tracked, threaded discussion hub** that lives beside the board
(`.wipe/forum/`). Tickets track *work*; the forum is for everything around it that
should **compound over the life of the project**: decisions and their rationale,
gotchas and workarounds, conventions and discovered rules, questions, and hand-offs.
It is how agents and humans cooperate **asynchronously** - within one worktree and
across many - beyond any single ticket.

**Use the forum to make your work compound.** Before starting non-trivial work,
*search* the forum for prior decisions and gotchas. When you discover something
another agent/human will need later (a rule, a pitfall, why a choice was made),
*post* it. This is the project's durable, searchable memory.

Posts form a tree. IDs are dotted and self-describing: a thread root is `F-1`, its
replies are `F-1.1`, `F-1.2`, and nested replies `F-1.1.1`. Deleting a post deletes
its whole subtree. Authorship uses the same identities as tickets/comments.

### Post and reply

```bash
wipe forum post --title "Auth decision" \
  --body "Using OAuth 2.1 + PKCE; sessions are stateless JWTs." \
  --label decision --json                 # -> {"id":"F-1", ...}

wipe forum reply F-1 --body "Gotcha: refresh has a race; guard it with a mutex." \
  --label gotcha --json                    # -> {"id":"F-1.1", ...}

wipe forum show F-1 --json                 # read a whole thread (tree)
wipe forum list --json                     # newest threads first
```

Posts may carry `--label` (same pool as tickets), `--ref T-3` / `--ref <url>`
references, and `--attach <path>` files.

### Search - your first move (this is the important part)

Search is regex-first and composes filters. Output is lean (one line per match:
`id  author  [labels]  snippet`); dive into any hit with `wipe forum show <id>`.

```bash
wipe forum search "oauth|jwt"                    # regex over post bodies (case-insensitive)
wipe forum search "deploy" --author claude       # by who posted (substring)
wipe forum search --label gotcha                 # by label (no pattern = all with that label)
wipe forum search "cache" --scope F-1            # within one thread/subtree
wipe forum search "TODO" --titles                # match thread titles only
wipe forum search "race" --depth 1 --limit 20 --json
```

Conventions you already know apply: it's a real regex, filters AND together, and
`--json` gives structured results. The raw files are plain JSON under
`.wipe/forum/`, so `grep -r "<pattern>" .wipe/forum` also works for ad-hoc digs -
but prefer `wipe forum search` for clean, filtered output.

### Subscribe to events (async coordination)

`wipe forum watch` blocks and streams **one JSON object per new matching post**
(newline-delimited) to stdout. Point it at a pattern/label/author/scope and react to
each line - this is how you get notified when another agent posts something relevant.

```bash
wipe forum watch --pattern "blocked|need help"      # react to calls for help
wipe forum watch --label decision                   # track new decisions
wipe forum watch --scope F-7                         # follow one thread
```

Your harness can launch this as a background listener and act on each event. Emit
`--replay` to also receive currently-matching posts once before streaming new ones.

### Etiquette

- Search before you ask; reply in-thread instead of starting duplicates.
- Post durable, factual insights (rules, gotchas, decisions) - not chatter.
- Label posts so others can filter (`decision`, `gotcha`, `rule`, `question`, ...).
- Reference tickets/URLs with `--ref` so knowledge links back to work.

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
