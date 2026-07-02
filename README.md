# wipe

**A git-native task board for humans and agents.**

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
- **Local desktop UX for humans.** Built with SvelteKit, shadcn-svelte, and
  Tauri, served by a lightweight local daemon (`wipe serve`).
- **Git-history board rewind.** Scrub through the board's past states and see
  GitLens-style attribution for who (human or agent) changed what, and when.
- **Flat, diffable JSON storage.** No embedded database — everything under
  `.wipe/` is designed to merge cleanly and read clearly in a diff.
- **Version-controlled media.** Attachments live in the repo alongside
  tickets, not in some external blob store.
- **One board per project.** `.wipe/` == board == project, created with a
  single `wipe init`.

## Install

`wipe` is not yet released; the following are the intended distribution
channels once binaries land at `0.1.0`.

**npm** (pure install wrapper that downloads the prebuilt binary):

```sh
npm install -g @mflrevan/wipe
```

**Cargo** (builds from source):

```sh
cargo install wipe
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
wipe ticket create "Write onboarding docs" --list Todo
```

Move it as work progresses:

```sh
wipe ticket move T-001 --list "In Progress"
```

Leave a comment for whoever (or whatever) picks it up next:

```sh
wipe comment add T-001 "Blocked on the API design ticket."
```

Check the board at a glance:

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
├── definitions.json    # labels, ticket schema, custom fields
├── settings.json       # local/project board settings
├── tickets/
│   ├── T-001.json
│   ├── T-002.json
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

## Project status

`wipe` is **pre-1.0 and under active development.** Commands, output formats,
and the on-disk `.wipe` schema may still change between releases. Feedback
and contributions are very welcome.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md) for how
to build, test, and submit changes, and [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)
for community expectations.

## License

`wipe` is dual-licensed under either of:

- MIT license ([LICENSE-MIT](./LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE))

at your option.
