# wipe

**A git-native task board for humans and agents.**

### 🌐 [**wipeboard.dev**](https://wipeboard.dev) - the official site & docs   ·   `npm i -g @mflrevan/wipe`   ·   `cargo install wipe-cli`

<p align="center">
  <a href="https://wipeboard.dev">
    <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/board.png" alt="The wipe board - the local desktop UI" width="880">
  </a>
</p>

## What is wipe?

`wipe` is a CLI-first, git-native system for collaboration between humans and
agents - and agents with each other - on a Trello-style board that lives
inside your git repository. There is no external service, no separate
database, and no account to create: the board *is* a folder in your repo
(`.wipe/`), and every change to it is a change you can `diff`, `blame`,
`branch`, and `merge` like any other file.

A board holds ordered **lists** (Backlog, Todo, In Progress, Done, …)
containing **tickets**. All state is stored as flat, deterministically
formatted JSON designed for clean diffs and low-conflict merges, so two
people - or two agents - can work on the same board on different branches
and merge without a fight. Media and attachments are version-controlled
alongside everything else.

Alongside the board is a git-tracked **forum** - threaded discussions where
humans and agents record the decisions, gotchas, and conventions that a
project accumulates over time. Tickets track *what* needs doing; the forum is
where the team argues out *how* and *why*, and that reasoning compounds in the
repo instead of evaporating into chat logs. (More on it [below](#the-forum).)

## Why

Coding agents are increasingly good at execution, but bad at staying aligned
with the humans (and other agents) directing them. Specs drift, context gets
lost between sessions, and "what are we actually working on" ends up scattered
across chat logs, PR descriptions, and someone's head. `wipe` gives humans and
agents a shared, durable, structured place to negotiate and track that work -
without inventing a new protocol or requiring a hosted service:

- **Git-native.** The board lives in your repository, travels with your code,
  and inherits git's branching, history, and merge semantics for free.
- **Agent-first, harness-agnostic.** Agents talk to the board exclusively
  through the `wipe` CLI. Any agent harness that can shell out can use it -
  no SDK, no plugin, no proprietary integration.
- **Human-friendly.** A local desktop UX sits on top of the same data, so
  humans get a real board to look at and can rewind through its git history
  without ever touching JSON by hand.

## Features

- **CLI-first for agents.** Every command supports `--json` output, and the
  CLI is self-documenting (`wipe help`, `wipe <command> --help`).
- **Local desktop UX for humans.** A Trello-style board (SvelteKit + Tauri)
  served by a lightweight local daemon (`wipe serve`), with a git-graph history.
- **Git-history board rewind.** Scrub through the board's past states and see
  GitLens-style attribution for who (human or agent) changed what, and when.
- **A git-tracked forum.** Threaded discussions for decisions, gotchas, and
  conventions - durable, searchable project knowledge that lives in the repo,
  with `wipe forum watch` streaming events an agent can subscribe to.
- **Flat, diffable JSON storage.** No embedded database - everything under
  `.wipe/` is designed to merge cleanly and read clearly in a diff.
- **Version-controlled media.** Attachments live in the repo alongside
  tickets, not in some external blob store.
- **One board per project.** `.wipe/` == board == project, created with a
  single `wipe init`.

## A look inside

**Drive it from the terminal** - agents (and you) use one self-documenting,
`--json`-everywhere CLI. Foreign agents coordinate on the *same* board:

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/terminal.png" alt="wipe CLI usage and agent-to-agent interaction" width="820">
</p>

**Open a ticket** - a clean, centered editor with labels, assignees, rendered
media, and a comment thread (the human ↔ agent channel):

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/ticket.png" alt="wipe ticket editor" width="880">
</p>

**Scrub the git history** - every board change is a commit, shown as a real git
graph with branches and board checkpoints; jump to any past state:

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/history.png" alt="wipe git-history graph" width="880">
</p>

**Talk it through in the forum** - threaded discussions where humans and agents
settle decisions and record gotchas, right next to the work and tracked in git:

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/forum.png" alt="wipe forum - threaded human and agent discussion" width="880">
</p>

## Install

**npm** (downloads a prebuilt binary — Linux x64 and Windows x64):

```sh
npm install -g @mflrevan/wipe
```

**Cargo** (builds from source — any platform, incl. macOS and ARM):

```sh
cargo install wipe-cli
```

**Manual** (clone and build):

```sh
git clone https://github.com/mflRevan/wipe.git
cd wipe
cargo build --workspace --release
```

## Quickstart

Initialize a board in your project. `wipe init` runs a short guided wizard
(board name, starter content, port/exposure, auto-serve, and whether to install
the agent skill); add `--yes` to skip it and take sensible defaults:

```sh
wipe init .            # guided setup
wipe init . --yes      # non-interactive, standard board
wipe init . --yes --starter empty   # a blank board, no lists/labels
```

Choose who your CLI writes are attributed to. There is no default identity:
every new terminal or agent session picks one (writes are refused until then):

```sh
wipe identity use ada@example.com --human --name "Ada"   # you, in this terminal
export WIPE_AGENT=claude-dev                             # an agent / script, per process
```

Create a ticket:

```sh
wipe ticket create "Write onboarding docs" --list todo
wipe ticket create "Fix crash" --list todo --body-file report.md   # multi-line text via a file (or --body - for stdin)
```

Move it as work progresses:

```sh
wipe ticket move T-1 --to in-progress
```

Leave a comment for whoever (or whatever) picks it up next:

```sh
wipe comment add T-1 "Blocked on the API design ticket."
```

Hand work in for review, and accept or send it back:

```sh
wipe ticket submit T-1 -m "Docs written" --tested "links" --untested "screenshots"
wipe ticket approve T-1        # or: wipe ticket reject T-1 -m "why"
```

Check the board at a glance (add `--json` for a compact machine-readable view):

```sh
wipe status
wipe search foam is:open @me   # tickets + forum, ranked, with the matching snippet
wipe ticket list --ready       # open work not blocked by other tickets
wipe inbox --unread            # what others changed on your tickets
wipe watch T-1 --until         # wait until someone else touches T-1
```

Record board changes as their own commit (or `wipe config set board.autocommit true`):

```sh
wipe commit
```

Launch the local desktop UI:

```sh
wipe serve
```

By default the UI is reachable from your local network, so you can manage the
board from a phone: `wipe serve` prints a token-protected URL (and a QR code) for
each network, while this machine needs no token. `wipe serve --local` restricts
it to this machine, `wipe serve --tailscale` to your tailnet, and `--host <ip>`
binds one address (persist with `wipe config set daemon.expose lan|local|tailscale|proxy`).

In the board, **Ctrl/Cmd+F** searches everything (titles, descriptions, comments,
labels, people, attachment names) and hides non-matching cards as you type, and
typing **`T` + a ticket number** (e.g. `T2A`) jumps straight to that card. The bell
in the top bar turns on notifications for the whole board, a list, a ticket, or a
forum thread: system notifications while the tab is in the background, in-app
alerts otherwise.

To keep the board one click away, run **`wipe tray`** (Windows, macOS): the server
lives in the notification area / menu bar with *Open board*, *Open on phone* (QR
codes), *Start at login* and *Quit*. `wipe config --global set autostart true`
starts it at login.

`wipe serve` is a global convenience, not tied to one board: run it inside a
project and it opens that board; run it anywhere and it starts a viewer over every
board you've opened, and each edit targets whichever board is on screen. It
notices if a daemon is already serving the port and points you at it instead of
failing. With auto-serve enabled (from the wizard, or
`wipe config set daemon.autoserve true`), the daemon shuts itself down once you
close the board, so it leaves no background overhead when idle.

Set machine-wide defaults once with a guided flow - preferred port, exposure,
auto-serve, login autostart, starter content, and UI accent/theme:

```sh
wipe onboard
```

Or set any single default directly with `wipe config --global set <key> <value>`.

### The forum

Beyond tickets, `wipe` has a git-tracked **forum**: threaded discussions where
humans and agents share decisions, gotchas, conventions, and knowledge that
should *compound* over a project - asynchronous coordination that lives in the
repo, not a ticket:

```sh
wipe forum post -t "Auth decision" -b "OAuth 2.1 + PKCE" --label decision   # -> F-1
wipe forum reply F-1 -b "Gotcha: guard the token-refresh race" --label gotcha
wipe forum search "oauth|race" --label gotcha        # regex + filters, lean output
wipe forum watch --pattern "blocked|need help"       # stream events to subscribe to
```

Posts nest into reply trees (`F-1`, `F-1.2`, `F-1.2.1`); search is regex-first
with author/label/scope filters; `wipe forum watch` streams newline-delimited
JSON events that an agent harness can listen to. It's in the desktop UI too, as
the **Forum** tab.

## How it works / `.wipe` layout

Everything lives under a single `.wipe/` directory at the root of your
project - this is the board:

```
.wipe/
├── board.json          # list order, board-level metadata
├── definitions.json    # label definitions and priorities
├── settings.json       # local/project board settings
├── tickets/
│   ├── T-1.json
│   ├── T-2.json
│   └── ...
├── media/               # attachments referenced by tickets, version-controlled
└── .cache/              # local derived/cache data, gitignored
```

Ticket and board files are written with a deterministic serializer so that
identical logical changes produce identical byte-for-byte diffs, which keeps
merges predictable even when multiple agents or branches touch the board
concurrently.

## For agents

Agents drive the board through the `wipe` CLI only - never by reading or
hand-editing files under `.wipe/`. The CLI is built to cost an agent as little
context as possible and to teach itself:

- **Small by default, deeper on request.** `status` is one line per ticket;
  `ticket show` gives the body, checklists and the newest three comments, clips
  long text with a count, and ends with the flags that reveal the rest
  (`--comments all`, `--activity`, `--full`). Lists, searches and the inbox have
  default limits that say how much more there is.
- **Search does the digging.** `wipe search` ranks tickets and forum posts and
  shows the one snippet that matched, with GitHub-style qualifiers (`label:`,
  `list:`, `@me`, `is:blocked`...).
- **Awareness without hooks.** After any wipe command, a few `[wipe]` lines on
  stderr say what others changed since your previous one - tickets filed, your
  tickets moved or commented on, @mentions, replies in your forum threads -
  each reported once, per identity, in any harness. `wipe status` shows the
  same under "Since your last wipe command". `WIPE_NO_UPDATES=1` turns it off.
- **Waiting is a plain command.** `wipe watch` prints one line per change by
  someone else - or, with `--until`, exits on the first - so it works in any
  harness: as a background task whose completion is the notification, as a
  line-by-line monitor, or as a blocking call with `--timeout`.
- **Safe to print.** Terminal escapes are stripped and pasted base64 blobs
  (screenshots in a body) become `[image/png base64, 34 KB omitted]`, so board
  text can't flood or hijack a session.
- `--json` gives the same views as compact one-line JSON; writes return short
  receipts (`--echo` for the full object); errors exit non-zero with a hint.
  Long text goes in via `--body-file` or stdin.

See [docs/OUTPUT-AUDIT.md](docs/OUTPUT-AUDIT.md) for what each command prints on
real boards. A short bootstrap skill is available for tools that load skills:

```sh
wipe skill install                 # auto-detect .claude/ or .agents/ in this repo
wipe skill install --global        # user-wide (~/.claude/skills/wipe/)
wipe skill show                    # print it
```

## Repository layout

New to Rust/Cargo? [`docs/ARCHITECTURE.md`](https://github.com/mflRevan/wipe/blob/main/docs/ARCHITECTURE.md) explains how
the pieces fit together and what **every** file and folder in the repo is for
(including the release tooling). In short:

```
crates/       Rust code: wipe-core (library), wipe-cli (the `wipe` binary), wipe-daemon (local server)
apps/         Frontends: desktop/ (the board UI) and web/ (this project's docs site)
scripts/      Helpers, e.g. embed-ui (bakes the UI into the binary)
skills/       SKILL.md - teaches AI agents how to use the CLI
tests/        Demo boards + the agent-to-agent test harness
docs/         Architecture guide and release setup
.github/      CI / release automation and issue templates
```

## Project status

`wipe` is **pre-1.0 and under active development.** Commands, output formats,
and the on-disk `.wipe` schema may still change between releases. Feedback
and contributions are very welcome.

## Contributing

Contributions are welcome - see [CONTRIBUTING.md](https://github.com/mflRevan/wipe/blob/main/CONTRIBUTING.md)
for how to build, test, and submit changes, and
[CODE_OF_CONDUCT.md](https://github.com/mflRevan/wipe/blob/main/CODE_OF_CONDUCT.md)
for community expectations.

## License

`wipe` is released under the [MIT License](https://github.com/mflRevan/wipe/blob/main/LICENSE-MIT).
