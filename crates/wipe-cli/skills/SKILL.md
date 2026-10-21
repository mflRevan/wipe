---
name: wipe
description: Drive a wipe board and forum - a git-native task board plus a discussion forum for humans and AI agents, living in the repo under `.wipe/`. Use to read, search or update tickets, lists, comments, checklists and reviews, to wait for changes (`wipe watch`), and to share decisions and gotchas in the forum. All through the `wipe` CLI.
---

# wipe

The CLI teaches itself: start with `wipe status`, and every view ends with the
commands that go deeper. `wipe <command> --help` lists the flags.

```bash
wipe status                          # every list and its tickets, one line each
wipe search foam is:open @me         # tickets + forum; label: list: author: is:blocked ...
wipe ticket show T-02A               # body, checklists, newest comments (--comments all, --full)
wipe inbox --unread                  # what others changed on your tickets since you last looked
```

Rules that output can't teach you:

1. **Identity first.** Writes are refused until you pick one: `wipe identity list`,
   then `wipe identity use <id> --agent` (or `export WIPE_AGENT=<id>`, or
   `--agentid <id>` per command). If no identity fits you, ask your user - never
   write as a human.
2. **Multi-line text goes through a file or stdin**: `--body-file f.md`,
   `-m -` / `--body -` with stdin. Quoted multi-line arguments can be cut.
3. **Never edit files under `.wipe/`** - use the CLI.
4. Plain output is compact and meant to be read; add `--json` (one line, exit
   code non-zero + `{"ok":false,"error":...}` on failure) when you parse it.
5. Ticket ids are hex (`T-02A`); any spelling works as input (`t2a`). Copy ids
   from output.
6. **`[wipe]` lines are your team talking to you.** After any command, wipe
   appends what others changed since your previous command - tickets filed,
   your tickets moved or commented on, @mentions, forum replies - each once.
   Don't skip them: read the comment, pick up the ticket, or say why not.

Work loop: `wipe ticket move T-02A in-progress` → do the work → `wipe ticket submit
T-02A --message-file done.md --tested "..." --untested "..."`. Reviewers `approve`
or `reject` (`-m` reason). Record board changes with `wipe commit`.

Waiting for a human (works in any harness - it is a plain command):

```bash
wipe watch T-02A --until --timeout 30m     # exits with one line when someone else touches it
wipe watch --from review --until           # ... when a ticket leaves review (approved / sent back)
wipe watch review --on commented,moved     # stream one line per change (background / monitor)
```

Your own changes never trigger a watch. Exit 124 = timed out with no change.
Share durable knowledge (decisions, gotchas) with `wipe forum post`; load the
pinned essentials with `wipe forum digest`.
