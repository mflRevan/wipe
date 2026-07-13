# Dogfood backlog → 0.3.10 (from running a 5-agent fleet on wipe)

Feedback gathered from five role lenses (dev, reviewer, researcher, marketing,
designer) after building a real app ("Huddle", ~108 tickets) coordinated entirely
on a wipe board + forum, with agents looping on a **shared single worktree** under
non-interactive PowerShell. Ordered by leverage; several downstream pains vanish
once the top items land.

## Shipped in 0.3.9
- Deletable ticket comments (`wipe comment remove`, daemon DELETE, UI).
- Ticket deletion from the UI (drag-to-trash) + list deletion unblocked.
- Forum UX overhaul: sort by last activity, richer sidebar rows (snippet, labels,
  author · relative-time · count), thread meta header, cleaner segmentation/accents.
- **`$WIPE_AGENT`** — a per-terminal, race-free agent identity (ranks above the
  shared session file), documented in SKILL.md as the multi-agent pattern. This is
  the contained fix for the #1 pain below.
- UI assignee resolution for `Name <email>` identities; UI comments attributed via
  the shared resolver (not the "ui" sentinel).

## 0.3.10 — carry-over big items (need design + live testing)

1. **Deeper identity isolation (beyond `$WIPE_AGENT`).** All 5 roles. Add
   `identity.pin = <id>` per-worktree config; a per-process owner-lock on
   `identity use` so a concurrent PID can't silently overwrite the binding;
   `WIPE_STRICT_IDENTITY` to hard-fail rather than misattribute; `whoami` warns when
   the resolved author differs from the last locally-bound one. Make `--agentid`
   uniform on every write subcommand (today `comment` has `--author` but `move`
   doesn't) and stop letting git-config be a silent fallback on shared boxes.

2. **wipe-owned atomic `.wipe/` commits.** All 5 roles. A concurrent agent's
   `git add -A && commit` repeatedly swept another agent's board state (and
   sometimes code) into the wrong commit/author, so `git log` no longer matched
   wipe's history. Add `wipe commit <ref>` (stages only that ticket's `.wipe/`
   files, attributed to the wipe identity) and opt-in `board.autocommit`.

3. **Real subscriptions + non-blocking inbox.** All 5 roles. `forum watch` blocks
   and is forum-only — useless in a 5-min loop. Add durable, process-independent
   subscriptions (`wipe subscribe ticket|list|forum <ref>`), auto-subscribe an
   assignee to their ticket, a per-identity read cursor, and **`wipe inbox --since
   <t>`** that returns-and-exits with everything changed on subscribed/assigned/
   authored objects (comments, list moves, assignments, replies). Eliminates the
   whole "idle-status" polling pattern.

4. **Author-correction verbs.** All 5 roles. `comment edit`/`comment reattribute`,
   `forum edit --author`, `ticket edit --author`, each writing an **audit entry**
   (not a silent overwrite). Removes the "Attribution note:" forum threads that were
   the single largest forum category once #1/#2 reduce their cause.

5. **Configurable default-create list + restricted lists.** All 5 roles. New tickets
   default to the *leftmost* list, which fights a PM-only leftmost "epics" column
   (we had to document a manual move-out dance). Add a `board.default_list` config
   (independent of order) and optionally mark lists as restricted/PM-only. (Pairs
   with the 0.3.10 draggable-lists work.)

## Smaller notes
- l10n: straight apostrophes break ICU plural/select parsing — validate/escape.
- Locale-variant content ids must not reuse the base-locale id prefix.
- `pumpAndSettle` spins forever against an infinite loading spinner (test guidance).
