---
name: wipe
description: Drive a wipe board and forum - a git-native, CLI-first task board plus a threaded discussion forum for humans and AI agents. Use to read or update tickets, lists, comments, labels, checklists, acceptance criteria, dependencies, reviews, and board state, AND to post/search/pin in the project forum where agents and humans share decisions, gotchas, conventions, and durable project knowledge. Works in any repo with a `.wipe/` directory (or run `wipe init` to create one). All interaction is through the `wipe` CLI with `--json`.
---

# wipe - agent operating guide

`wipe` is a git-native task board that lives inside a repository under `.wipe/`.
Humans usually work it from the web UI (`wipe serve`); you work it from the CLI.

## Start of every session (do this first)

Your context may have been reset since you last looked. Catch up in three cheap
calls instead of reading the whole board:

```bash
wipe identity whoami --json               # who am I here? (null = choose one, see below)
wipe inbox --unread --json                # what others changed on my tickets since I last looked
wipe ticket list --exclude-list done --json   # compact rows: id, title, list, labels, blockers...
```

Then drill into single tickets with `wipe ticket show T-7 --json`. Useful narrower
views: `wipe ticket list --list todo --json`, `--ready` (not waiting on anything),
`--assignee me`, `--since 1d`, `--fields id,title`. `wipe inbox --all --json` shows
everything anyone else changed, not just your tickets. After a long break, that
plus `wipe forum digest` is the whole catch-up.

## Golden rules

1. Add `--json` to every command. Output is ONE compact JSON line on stdout (add
   `--pretty` only if you need to eyeball it).
2. Exit code `0` = success. On failure it is non-zero and stdout is
   `{"ok": false, "error": "..."}` - the error says how to fix the call.
3. Writes answer with a short receipt (`{"ok":true,"id":"T-3","list":"review"}`).
   Pass `--echo` only when you need the full updated object back.
4. **Multi-line text goes through a file or stdin, never a quoted argument:**
   `--body-file notes.md`, or `--body -` with the text on stdin. (Through
   Windows' `wipe.cmd`/`cmd.exe`, an argument is silently cut at its first line
   break.) Same for `--comment-file`, `--message-file`, forum `--body-file`.
5. Never read or write files under `.wipe/` - their format is internal and
   differs from the CLI's JSON. Everything you need is a command away.
6. IDs are stable: tickets `T-<n>`, comments `c-<n>`, checklist `ck-<n>`,
   criteria `ac-<n>`, forum posts `F-<n>` / `F-<n>.<m>`, lists are slugs
   (`in-progress`) that survive renames.
7. Unsure about a flag? `wipe <group> --help`.

## Identity - required before any write

Every write (create, edit, comment, move, post, ...) is attributed to an
identity, and **there is no default**: until you choose one, writes are refused
with a list of the board's identities. Reads never need one.

```bash
wipe identity list --json                 # existing identities (humans + agents)
wipe identity use claude --agent --name "Claude" --json   # bind to THIS session
wipe identity whoami --json               # {"identity":"claude","source":"session ..."}
```

- A binding lasts for this agent session (or terminal tab). A new session starts
  with no identity again - just re-run `wipe identity use <you>`.
- In harnesses that start a fresh shell per command, or when several agents share
  one machine, pin it per process instead: `export WIPE_AGENT=claude-dev`
  (PowerShell: `$env:WIPE_AGENT = "claude-dev"`), or pass `--agentid <id>` on a
  single command.
- **If no listed identity fits you, stop and ask your user which one to use.**
  Never guess, and never write as a human's identity.

## Everyday flows

```bash
wipe ticket create "Add login" --list todo --label app --json      # -> {"id":"T-1",...}
wipe ticket create "Fix crash" --list todo --body-file report.md --json
wipe ticket move T-1 --to in-progress --json
wipe comment add T-1 "Spec clarified: OAuth" --json                # short text inline is fine
wipe comment add T-1 --body-file findings.md --json                # anything multi-line
wipe label assign T-1 app bug --json                               # several labels at once
```

**Change many things in one call** - title, body, labels, assignees, blockers,
list and a comment together (validated before anything is written):

```bash
wipe ticket edit T-4 -t "Proper title" --body-file plan.md \
  --label app --remove-label raw --assignee claude \
  --blocked-by T-2 --to todo -m "Rewrote Bilal's note into a ticket" --json
```

When you rewrite a ticket someone else created, their original title/body is
kept automatically (`original` in `ticket show`) - rewriting a human's raw note
is safe; quote it anyway if you refer to it.

`wipe ticket show T-1 --json` also lists commits whose messages mention `T-1`,
so put ticket ids in your commit messages (`fix login redirect (T-1)`). Trim big
tickets with `--comments 5`, `--comments-only`, `--no-activity`.

## The review loop (human <-> agent)

```bash
# you, when the work is done:
wipe ticket submit T-1 -m "Implemented OAuth device flow" \
  --tested "unit + integration tests" --untested "iOS device" --json
#   -> comment with Tested / Not tested sections, moved to the review list
# the reviewer (human or agent):
wipe ticket approve T-1 -m "Looks good" --json    # -> done (refuses while criteria are unticked; --force)
wipe ticket reject T-1 -m "Crashes on launch: steps..." --json   # reason required -> back to todo
```

A rejection lands in your `wipe inbox` with the reason. Acceptance criteria are the
definition of done: read them before starting (`wipe criteria list T-1`), the
reviewer ticks them (`wipe criteria check T-1 ac-1`). Workflow lists are
detected (a list named like "review", `done`, `todo`) or configured:
`wipe config set board.review_list <list>` (also `board.done_list`, `board.rework_list`).

## Dependencies - "what can be done next?"

```bash
wipe ticket block T-5 --by T-3 --json      # T-5 waits on T-3 (cycles are refused)
wipe ticket list --ready --json            # open work not waiting on an open blocker
wipe ticket list --blocked --json
wipe ticket unblock T-5 --by T-3 --json
```

A blocker stops counting once it reaches the done list.

## Checklists and acceptance criteria

```bash
wipe checklist add T-1 --text "Write integration tests" --json
wipe checklist check T-1 ck-1 --json        # also: uncheck, toggle, edit, move, remove
wipe criteria add T-1 --text "All tests pass in CI" --json
```

## Committing board changes

Board changes are plain files under `.wipe/`. Record them separately from your
code so a human's UI edits never ride along in your commits:

```bash
wipe commit --json                  # one commit of .wipe/ only, authored as you
wipe commit T-3 -m "spec T-3" --json
wipe config set board.autocommit true   # or: commit after every write automatically
```

Run `wipe commit` before your own `git add -A` (or add paths explicitly).
Until `wipe commit` has been used once on a machine, writes print a reminder on
stderr. `wipe doctor --json` reports uncommitted board files.

## The forum - durable project knowledge

Tickets track work; the forum holds what should outlive it: decisions, gotchas,
conventions. Search before you start, post what you learn.

```bash
wipe forum search "oauth|jwt" --json              # regex; --label, --author, --scope F-1
wipe forum post -t "Auth decision" --body-file decision.md --label decision --json
wipe forum reply F-1 --body "Gotcha: refresh races; guard it." --json
wipe forum show F-1 --depth 1 --json
wipe forum pin F-1 --json                          # pinned threads form the digest
wipe forum digest                                  # compact Markdown of pinned threads (<= 4000 bytes)
```

`wipe forum digest` is built to be loaded into your context at session start
(e.g. from a hook or referenced in CLAUDE.md). Pin the few threads that every
session should know.

## Inbox and subscriptions

`wipe inbox` returns activity by *others* on tickets you're assigned to,
authored, or subscribed to, newest first (capped at 50; `total` has the full
count). `--unread` shows only what's new since you last used it and marks it read.

```bash
wipe subscribe todo --json          # a list; also T-3, F-2, forum, all
wipe inbox --unread --json
wipe inbox --since 2026-09-01 --json
```

## Deleting and restoring

`wipe ticket delete T-1 --yes` moves a ticket to the restorable trash
(`wipe trash list|restore|purge`); `--purge` deletes for good.

## Output size

Compact views are designed for agent context: see `docs/OUTPUT-AUDIT.md` in the
wipe repo for measured sizes. Avoid `status --full` / `ticket list --full` /
`inbox --limit 0` in loops - they grow with the entire board.
