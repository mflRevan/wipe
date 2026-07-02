# wipe

**A git-native task board for humans and agents.**

### 🌐 [**wipeboard.dev**](https://wipeboard.dev) — the official site & docs   ·   `npm i -g @mflrevan/wipe`   ·   `cargo install wipe-cli`

<p align="center">
  <a href="https://wipeboard.dev">
    <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/board.png" alt="The wipe board — the local desktop UI" width="880">
  </a>
</p>

## What is wipe?

`wipe` is a CLI-first, git-native system for collaboration between humans and
agents — and agents with each other — on a Trello-style board that lives
inside your git repository. There is no external service, no separate
database, and no account to create: the board *is* a folder in your repo
(`.wipe/`), and every change to it is a change you can `diff`, `blame`,
`branch`, and `merge` like any other file.

A board holds ordered **lists** (Backlog, Todo, In Progress, Done, …)
containing **tickets**. All state is stored as flat, deterministically
formatted JSON designed for clean diffs and low-conflict merges, so two
people — or two agents — can work on the same board on different branches
and merge without a fight. Media and attachments are version-controlled
alongside everything else.

## Why

Coding agents are increasingly good at execution, but bad at staying aligned
with the humans (and other agents) directing them. Specs drift, context gets
lost between sessions, and "what are we actually working on" ends up scattered
across chat logs, PR descriptions, and someone's head. `wipe` gives humans and
agents a shared, durable, structured place to negotiate and track that work —
without inventing a new protocol or requiring a hosted service:

- **Git-native.** The board lives in your repository, travels with your code,
  and inherits git's branching, history, and merge semantics for free.
- **Agent-first, harness-agnostic.** Agents talk to the board exclusively
  through the `wipe` CLI. Any agent harness that can shell out can use it —
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
- **Flat, diffable JSON storage.** No embedded database — everything under
  `.wipe/` is designed to merge cleanly and read clearly in a diff.
- **Version-controlled media.** Attachments live in the repo alongside
  tickets, not in some external blob store.
- **One board per project.** `.wipe/` == board == project, created with a
  single `wipe init`.

## A look inside

**Drive it from the terminal** — agents (and you) use one self-documenting,
`--json`-everywhere CLI. Foreign agents coordinate on the *same* board:

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/terminal.png" alt="wipe CLI usage and agent-to-agent interaction" width="820">
</p>

**Open a ticket** — a clean, centered editor with labels, assignees, rendered
media, and a comment thread (the human ↔ agent channel):

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/ticket.png" alt="wipe ticket editor" width="880">
</p>

**Scrub the git history** — every board change is a commit, shown as a real git
graph with branches and board checkpoints; jump to any past state:

<p align="center">
  <img src="https://raw.githubusercontent.com/mflRevan/wipe/main/docs/images/history.png" alt="wipe git-history graph" width="880">
</p>

## Install

**npm** (downloads the prebuilt binary for your platform):

```sh
npm install -g @mflrevan/wipe
```

**Cargo** (builds from source):

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

Initialize a board in your project:

```sh
wipe init .
```

Create a ticket:

```sh
wipe ticket create --title "Write onboarding docs" --list todo
```

Move it as work progresses:

```sh
wipe ticket move T-1 --to in-progress
```

Leave a comment for whoever (or whatever) picks it up next:

```sh
wipe comment add T-1 --body "Blocked on the API design ticket."
```

Check the board at a glance (add `--json` for machine-readable output):

```sh
wipe status
```

Launch the local desktop UI:

```sh
wipe serve
```

## How it works / `.wipe` layout

Everything lives under a single `.wipe/` directory at the root of your
project — this is the board:

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

Agents are expected to interact with the board exclusively through the
`wipe` CLI — never by hand-editing files under `.wipe/`. Every command
accepts a `--json` flag for structured, machine-parseable output, and the
CLI's built-in help (`wipe help`) is written to be sufficient documentation
on its own. Repositories using `wipe` may also ship a `SKILL.md` describing
project-specific conventions for how agents should use the board.

## Repository layout

New to Rust/Cargo? [`docs/ARCHITECTURE.md`](https://github.com/mflRevan/wipe/blob/main/docs/ARCHITECTURE.md) explains how
the pieces fit together and what **every** file and folder in the repo is for
(including the release tooling). In short:

```
crates/       Rust code: wipe-core (library), wipe-cli (the `wipe` binary), wipe-daemon (local server)
apps/         Frontends: desktop/ (the board UI) and web/ (this project's docs site)
scripts/      Helpers, e.g. embed-ui (bakes the UI into the binary)
skills/       SKILL.md — teaches AI agents how to use the CLI
tests/        Demo boards + the agent-to-agent test harness
docs/         Architecture guide and release setup
.github/      CI / release automation and issue templates
```

## Project status

`wipe` is **pre-1.0 and under active development.** Commands, output formats,
and the on-disk `.wipe` schema may still change between releases. Feedback
and contributions are very welcome.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](https://github.com/mflRevan/wipe/blob/main/CONTRIBUTING.md)
for how to build, test, and submit changes, and
[CODE_OF_CONDUCT.md](https://github.com/mflRevan/wipe/blob/main/CODE_OF_CONDUCT.md)
for community expectations.

## License

`wipe` is released under the [MIT License](https://github.com/mflRevan/wipe/blob/main/LICENSE-MIT).
