# Changelog

## 0.4.1 - 2026-10-01

### Ticket ids

- **New boards use fixed-width hex ticket ids:** `T-001` ... `T-0FF`, `T-2AF`
  (three or more upper-case hex digits; `board.json` records `"ids": "hex"`).
  Short, uniform, and quick to type.
- **Any spelling resolves everywhere** (CLI, API, quick-jump): `T-02A`, `t2a`,
  `T-2A`, `T 2a`. CLI output always shows the canonical id.
- **Legacy boards keep working** with decimal ids (`T-23`). `wipe board
  translate-ids` (or the **New ids** button in the board UI, shown only on such
  boards) converts one: ticket files are renamed, and cards, relations,
  subscriptions, trash entries, forum refs and `T-<n>` mentions in titles,
  bodies, comments, checklists and activity are rewritten. Each ticket keeps its
  old id as `legacy_id`, so `T-23` (in old notes or commit messages) still
  resolves, and `ticket show` also lists commits that mention the old id. The
  first CLI run on a legacy board offers the translation once: an interactive
  y/N question, or a one-line stderr notice for agents and scripts (never a
  blocking prompt). The staged translation is crash-safe.

### Board UI

- **Ticket quick-jump:** with nothing focused, type `T` and the ticket's digits
  (`T2A`; the dash is optional). The query shows large in the middle of the
  screen, the matching card glows and scrolls into view, Enter opens it, Escape
  or any click dismisses. Ticket ids are now shown on every card.
- **Board search (Ctrl/Cmd+F)** replaces the browser's find-in-page: it searches a
  cached index of every ticket's id (and pre-translation id), title, description,
  labels, priority, assignees and authors (ids and display names), comments,
  checklist and acceptance items, and attachment names; all terms must match,
  "quoted phrases" stay together, accents are ignored. Non-matching cards hide
  live, per-list counts show `3/108`, hits in titles and labels are highlighted,
  and cards that matched elsewhere say so. The search survives opening tickets;
  Escape or the close button (marked `esc`) ends it. In the forum view, Ctrl/Cmd+F
  focuses the forum's own search.
- **Notifications:** watch the whole board, a list (list menu), a ticket (bell in
  the ticket header), a forum thread, or the whole forum (the bell menu in the
  top bar). Changes by others - new cards, moves (into or out of a watched
  list), comments, assignments, labels, edits, completed checklists or
  criteria, forum replies - arrive as native system notifications while the
  tab is in the background (click to open the ticket or thread) and as in-app
  toasts while you're looking; bursts collapse into one summary. Your own edits
  never notify you. Watches are kept per board in the browser. System
  notifications need https or localhost; on a LAN address (e.g. a phone) the
  in-app alerts are used.
- **Faster, smoother board on large boards** (measured on a 127-ticket board
  with 1.1 MB of tickets, dragging a card across a 108-card list):
  - the board poll uses ETags: the daemon caches the payload by file
    fingerprint and answers unchanged polls with `304` (0 bytes, ~1 ms) instead
    of re-reading every ticket and sending ~730 KB every half second, and the
    UI no longer `JSON.stringify`s the whole board twice per poll to detect
    changes;
  - the daemon also listens on `::1` / `::`, so `http://localhost` no longer
    waits ~200 ms per request for a failed IPv6 attempt before falling back to
    IPv4 (most noticeable on Windows);
  - unchanged tickets keep their identity across refreshes, the board keeps its
    lists as raw (non-proxied) state, and reorder animations run only for cards
    in view.
  Result: the drag that froze 0.4.0 for 389 ms now has no long tasks (worst
  frame 49 ms), and search filtering settles within a frame or two.

### Tray app

- **`wipe tray`** (Windows, macOS) runs the board server as a real desktop app:
  an icon in the notification area / menu bar whose menu opens the board, opens
  a phone page with QR codes for every network address, copies the phone link,
  toggles start-at-login, and quits cleanly. Left-click opens the board. Only
  one runs per user (a second `wipe tray` just opens the board). Started from a
  terminal it moves to the background. Login autostart now launches the tray on
  Windows and macOS instead of a hidden `wipe serve`; Linux keeps the systemd
  user unit.
- The daemon serves `/connect` (this machine only): the board's network URLs
  with the access token as links and QR codes; `wipe serve` points at it.

### Fixed

- `wipe serve` / `wipe scan` find boards on every local drive, not just under
  your home folder: the default scan roots are now home plus each fixed drive
  (`D:\`, `E:\`, ...; mounted volumes under `/Volumes`, `/mnt`, `/media` on
  macOS/Linux). Network and removable drives are left out; OS trees at drive
  roots (`Windows`, `Program Files`, `$Recycle.Bin`, ...) and `AppData` are
  skipped, and a root inside another (home inside `C:\`) is walked once.
  `wipe serve` now also scans when started inside a board (in the background,
  so startup stays instant), and the UI's project switcher refreshes its list
  each time it opens. Configured `scan.roots` still replace the defaults.
- `wipe trash restore|purge` built the trash file path straight from the typed
  id, so `wipe trash purge ../../board` could delete `.wipe/board.json`; trash
  ids are now matched against the trash entries themselves (any id spelling).
- UI: dropping a card on the trash bin no longer flies it back to its list for a
  moment before it disappears. The drop-return animation is skipped while the
  pointer is over the bin, and the card leaves the board immediately (the delete
  itself was already correct).

## 0.4.0 - 2026-09-29

Driven by twelve days of field feedback from an agent (Claude) and a human
sharing one board on a real project, plus human-side QoL work. The theme: less
output, safer text input, forced identity, a first-class review loop, and a board
you can run from your phone.

### Breaking changes

- **No default identity.** CLI writes are refused until an identity is chosen for
  the session (`wipe identity use <id>`, `$WIPE_AGENT`, `--agentid`, a per-command
  `--author`, or `$WIPE_AUTHOR`). The repo's VCS user, the board's
  `default_author`, and `identity.default` are no longer used by the CLI (the
  board UI still uses them). The refusal lists the board's identities, how to pick
  one, and tells agents to ask their user rather than guess. Reads never need an
  identity. `$WIPE_STRICT_IDENTITY` is now redundant (always strict).
- **Sessions are per terminal tab / agent session.** The shared `default` session
  key is gone (it made one `identity use` apply to every terminal and every agent
  on the machine). Keys come from `$WIPE_SESSION`, the agent harness's session id
  (e.g. Claude Code), the terminal's tab id (Windows Terminal, iTerm, tmux, ...),
  or the launching shell process. An agent started from a human's terminal never
  inherits the human's identity. Old bindings are ignored.
- **Compact JSON.** `--json` prints one compact line (about half the bytes);
  `--pretty` restores indentation. Object keys now keep their natural order.
- **`wipe status --json` is compact:** per list `{id, name, count, tickets:[row]}`
  with short rows; the done list collapses to `{count, collapsed:true}`. Use
  `--all` to expand done and `--full` for the old whole-content dump (whose list
  key is still `list`).
- **`wipe ticket list --json` returns compact rows** (`--full` for whole tickets).
- **Write commands return short receipts** (`{"ok":true,"id":"T-3","list":"todo"}`,
  37-80 bytes) instead of the whole ticket; `--echo` returns the full object.
- **`wipe serve` listens on all local networks by default** (see below). Remote
  clients need the access token; this machine does not. `daemon.expose` values are
  now `lan | local | tailscale | proxy`; the old `none` is read as `lan` and
  rejected by `config set` (use `local`).
- `wipe inbox` and `wipe forum search` return at most 50 items by default
  (`--limit 0` for all; inbox JSON adds `total`).
- MSRV is now Rust 1.89.

### Agent experience

- **Long text from files or stdin** everywhere text goes in: `--body-file <path>` /
  `--body -` on `ticket create|edit`, `comment add|edit`, `forum post|reply|edit`;
  `--comment-file` on `ticket edit`; `--message-file` on `ticket submit|reject`.
  File/stdin text loses a UTF-8 BOM and trailing newlines. Fixes silently
  truncated multi-line bodies on Windows.
- **Windows npm shim:** global npm installs now also place the native `wipe.exe`
  next to npm's shims, so `wipe` resolves to the binary instead of `wipe.cmd`
  (cmd.exe cuts arguments at the first line break). Launches through the node
  shim are marked, and `wipe doctor` explains the risk and the fix.
- **`wipe ticket edit` does everything in one call:** title, body, priority,
  `--label`/`--remove-label`, `--assignee`/`--unassign`, `--blocked-by`/`--unblock`,
  `--to <list>`, and `-m` comment - validated before anything is written.
- **Positional forms:** `wipe ticket create "Title" --list todo`,
  `wipe comment add T-1 "text"`; `wipe label assign|remove T-1 app bug` takes
  several labels; `-t`/`-b` short flags accepted on `ticket edit`.
- **`wipe ticket list` filters:** repeatable `--list` / `--exclude-list`,
  repeatable `--label` (all must match), `--assignee <who|me>`,
  `--since <RFC-3339 | 2026-09-01 | 30m | 12h | 2d | 1w>`, `--ready`, `--blocked`,
  `--fields id,title,...`, `--limit`. Unknown lists/fields fail with the valid set.
- **`wipe ticket show`:** lists commits whose messages mention the ticket id
  (`--no-commits`); `--comments N`, `--comments-only`, `--no-activity` trim it.
- **Original notes are kept:** the first time someone other than the creator
  rewrites a ticket's title or body, the creator's wording is frozen in `original`
  (shown by `ticket show`), so turning a human's raw note into a ticket is safe.
- **Status footer / hints:** human `status` ends with unread count and a pointer
  to `ticket list`; JSON status carries `identity` and `unread`.
- **Commit story:** until `wipe commit` has been used once on a machine, every
  board write prints a one-line stderr hint to record it (stdout JSON untouched);
  `wipe doctor` reports uncommitted board files. (`board.autocommit` remains.)
- **`board.json` churns less:** card moves, creates, deletes and trash no longer
  rewrite `board.updated`, so parallel worktrees conflict less.
- **`wipe inbox --all`:** everything anyone else changed on the board (reason
  `board`), for "what happened while I was away"; `--since` accepts the same
  forms as `ticket list`.
- **Forum digest:** `wipe forum pin|unpin F-n` and `wipe forum digest
  [--max-bytes 4000]` - a size-bounded Markdown summary of pinned threads, made to
  be loaded into an agent's context at session start.
- **Skill guide rewritten** around a start-of-session recipe (`whoami` → `inbox
  --unread` → `ticket list --exclude-list done`), the identity rules, file/stdin
  text, receipts, the review loop, dependencies, and committing; it now states
  that `.wipe/` files are internal.

### Human + agent workflow

- **Review loop:** `wipe ticket submit T-1 -m "..." --tested ... --untested ...`
  (comment with Tested / Not tested sections + move to the review list + the
  submitter is subscribed), `wipe ticket approve T-1 [-m] [--force]` (refuses while
  acceptance criteria are unticked), `wipe ticket reject T-1 -m "reason"` (reason
  required; back to the rework list). Workflow lists are detected (a "review" list,
  `done`, `todo`) or set with `board.review_list`, `board.done_list`,
  `board.rework_list`.
- **Dependencies:** `wipe ticket block|unblock T-5 --by T-3`, `--blocked-by` on
  create/edit; cycles are refused; blockers stop counting once done;
  `ticket list --ready` / `--blocked`; rows and `status` show open blockers.

### `wipe serve` and the phone

- Default exposure is the local network (`0.0.0.0`) with token auth for remote
  clients; requests from this machine skip the token (except in `proxy` mode).
  New flags: `--expose <mode>`, `--local`, `--tailscale` (binds loopback + this
  machine's Tailscale address, prints the MagicDNS URL when available),
  `--host <ip>`, `--no-qr`.
- Prints one URL per network interface and a terminal **QR code** for the phone.
- The access token is kept per machine in the user config dir (`serve-token`),
  never in the git-tracked board (a token already in `settings.json` is honored).
- Cross-origin API access in exposed mode is limited to this machine's own
  front-ends (desktop app, local dev server).
- **Mobile UI:** two-row header, one list per screen with swipe snapping, touch
  drag only after a short press (so swipes scroll), card menus visible without
  hover, full-screen ticket and create dialogs, single-pane forum with a back
  button, safe-area aware.

### UI

- The ticket title field grows with its content while editing (all wrapped lines
  visible) in both the ticket modal and the create dialog; the description editor
  grows too.
- Pasted or picked media shows an **in-place preview** before the card exists
  (images, video, audio, text/PDF tiles) and right under the field it was pasted
  into on existing cards; tap any preview or attachment to inspect it full-screen
  (zoom, download). Pasted local paths preview via the daemon (this machine only).
  Files can be dropped onto the create dialog.

### Reliability and security

- **Board write lock:** every CLI write and every UI mutation holds a per-board
  lock, so concurrent agents / the UI can no longer lose tickets, lose comments,
  or allocate the same ticket id (all three reproduced by the new stress tests
  without the lock).
- Attaching a file *by path* (and the new path preview) is refused for remote
  clients, so a token holder on the network can't make the daemon read arbitrary
  host files.
- Remote clients can't claim an identity in exposed mode; local ones can.

### Tests and docs

- New integration suites: `agent_ux` (14 end-to-end scenarios for the features
  above) and `stress` (concurrent creators/commenters/mixed writers, output-size
  bounds on a 300-ticket board); shared test harness that scrubs ambient identity
  and session env.
- New unit tests for commit mention matching, blockers/cycles, original notes,
  workflow list detection, `board.updated` stability, `--since` parsing, field
  selection, serve bind plans / URLs / QR, loopback trust, and CORS origins.
- `docs/OUTPUT-AUDIT.md`: measured bytes / tokens / time for every routine command
  at 10 / 100 / 500 tickets, plus every truncation and compaction rule; regenerate
  with `cargo test -p wipe-cli --release --test stress -- --ignored output_audit`.
- README, website CLI reference, and harness scripts updated for 0.4.
