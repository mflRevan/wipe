# Architecture & repository guide

This document explains how the wipe project is put together and what every file
and folder in the repo is for. It's written to be readable even if you're new to
Rust/Cargo tooling.

## The big picture

wipe has one shared core and several things built on top of it:

```
                        ┌─────────────────────────┐
                        │        wipe-core         │  the library: data model,
                        │  (.wipe read/write, git, │  JSON storage, git history.
                        │      board operations)   │  Everyone depends on this.
                        └────────────┬────────────┘
              ┌──────────────────────┼──────────────────────┐
              │                      │                       │
      ┌───────▼───────┐     ┌────────▼────────┐              │
      │   wipe-cli    │     │   wipe-daemon   │              │
      │ the `wipe`    │     │ local HTTP/WS   │              │
      │ command       │     │ server + API    │              │
      └───────────────┘     └────────┬────────┘              │
        agents + humans               │ serves               │
        run this                      │                       │
                             ┌────────▼────────┐     ┌────────▼────────┐
                             │  apps/desktop   │     │    apps/web     │
                             │ SvelteKit board │     │  React docs +   │
                             │ UI (+ Tauri)    │     │  marketing site │
                             └─────────────────┘     └─────────────────┘
```

- **Agents** only ever use `wipe-cli` (the `wipe` command). That's the contract.
- **Humans** run `wipe serve`, which starts `wipe-daemon`; it serves the
  **desktop board UI** (built from `apps/desktop`) in a browser or a native
  window.
- **`apps/web`** is a separate public website (what wipe is + docs). It is *not*
  part of the app you run locally.

## What "Cargo workspace" and "crate" mean

Rust's build tool is called **Cargo**. A **crate** is a Rust package (like an npm
package). A **workspace** is a set of crates that live in one repo and are built
together. wipe's workspace has three crates under `crates/`:

| Crate | Kind | What it does |
| --- | --- | --- |
| `wipe-core` | library | The heart: the data types (Board, List, Ticket, …), reading/writing the `.wipe/` JSON files deterministically, git history, and the board operations. No other crate touches `.wipe` files directly. |
| `wipe-cli` | binary (`wipe`) | The command-line tool. Thin layer over `wipe-core`; defines all the `wipe …` subcommands. |
| `wipe-daemon` | library | The local web server (`wipe serve`) that exposes the board over HTTP/WebSocket and serves the human UI. |

## The two frontends (JavaScript, built with pnpm)

These are normal web apps built with **pnpm** (a Node package manager) — separate
toolchain from Rust.

- **`apps/desktop`** — the human board UI (SvelteKit). Its production build is
  *embedded into the `wipe` binary* by `scripts/embed-ui`, so `wipe serve` can
  serve it with no extra install. It also has a `src-tauri/` folder to optionally
  wrap it as a native desktop app.
- **`apps/web`** — the public website/docs (React). Deployed as a static site.

## The `.wipe/` data format

Each project that runs `wipe init` gets a `.wipe/` folder committed to git:

```
.wipe/
  board.json         # the lists (columns) and the order of cards in each
  definitions.json   # ticket types, labels, tags, priorities
  settings.json      # daemon port + exposure
  tickets/T-1.json   # one file per ticket (its fields + inline comments)
  media/             # attachments (version-controlled)
  .cache/            # local SQLite cache, gitignored (safe to delete)
```

Because each ticket is its own file and card *ordering* is kept separately in
`board.json`, two people editing different tickets never produce a git conflict.

## Every root file, explained

Most of these are standard, required config files — they look like clutter but
each has a specific job and generally must live at the repo root.

### Rust / build

| File | Why it exists |
| --- | --- |
| `Cargo.toml` | The **workspace manifest**: lists the crates, shared settings (version, license, authors), and shared dependency versions. The main "project file" for the Rust side. |
| `Cargo.lock` | Auto-generated. Pins the *exact* versions of every dependency so builds are reproducible. Committed on purpose; you don't edit it by hand. |
| `rust-toolchain.toml` | Pins the Rust compiler version/components so everyone (and CI) builds with the same toolchain. |
| `rustfmt.toml` | Settings for `cargo fmt`, the Rust code formatter (line width, etc.). |

### Release automation

| File | Why it exists |
| --- | --- |
| `dist-workspace.toml` | Config for **cargo-dist** — a tool that, when you push a version tag, builds the `wipe` binary for every OS (Linux/macOS/Windows), makes install scripts, and attaches them to a GitHub Release. |
| `release-plz.toml` | Config for **release-plz** — see "What is release-plz" below. |

### Community / legal / meta

| File | Why it exists |
| --- | --- |
| `README.md` | The front page: what wipe is, install, quickstart. |
| `LICENSE-MIT`, `LICENSE-APACHE` | The open-source license (dual MIT/Apache-2.0, the Rust-ecosystem norm — users may pick either). |
| `CONTRIBUTING.md` | How to build/test and the commit conventions. |
| `CODE_OF_CONDUCT.md` | Community behavior policy (standard Contributor Covenant). |
| `SECURITY.md` | How to report a security issue privately. |
| `.gitignore` | Files git should never track (build output, `node_modules`, local caches). |
| `.gitattributes` | Normalizes line endings to LF across OSes so diffs stay clean. |

### Folders

| Folder | Contents |
| --- | --- |
| `crates/` | The three Rust crates (see above). |
| `apps/` | The two frontends: `desktop/` (board UI) and `web/` (docs site). |
| `scripts/` | Helper scripts, e.g. `embed-ui` (build the UI into the binary). |
| `skills/` | `SKILL.md` — the guide that teaches AI agents how to drive the CLI. |
| `tests/` | Cross-cutting tests & fixtures: `mock-projects/` (demo boards) and `agent-supervision/` (the agent-to-agent harness). Per-crate unit tests live inside each crate. |
| `docs/` | This guide and `RELEASE.md`. |
| `.github/` | GitHub config: CI/release **workflows** (automation that runs on GitHub) and issue/PR templates. |

## What is release-plz?

[`release-plz`](https://release-plz.dev) is a release-automation tool for Rust.
You never have to hand-edit version numbers or changelogs. It works from your
commit messages (which follow the **Conventional Commits** style, e.g.
`feat: …`, `fix: …`, `chore: …`):

1. On every push to `main`, release-plz looks at the new commits.
2. It opens (or updates) a **"Release PR"** that bumps the version numbers and
   writes a `CHANGELOG.md` entry describing what changed.
3. When *you* merge that PR, release-plz tags the release (e.g. `v0.1.0`) and
   publishes the Rust crates to **crates.io** (Rust's package registry).
4. That new tag triggers **cargo-dist**, which builds the downloadable binaries
   and the `@mflrevan/wipe` npm package.

So the whole release is: **write conventional commits → merge the Release PR → everything else is automated.** The one-time setup (tokens, etc.) is in
[`docs/RELEASE.md`](./RELEASE.md).

## The two toolchains, side by side

- **Rust side:** `cargo build` (compile), `cargo test` (test), `cargo fmt`
  (format), `cargo clippy` (lint). Config: `Cargo.toml`, `rust-toolchain.toml`,
  `rustfmt.toml`.
- **Frontend side:** `pnpm install` then `pnpm build` inside `apps/web` or
  `apps/desktop`. Config: each app's `package.json`.

`scripts/embed-ui` is the bridge: it runs the frontend build and copies the
result into the daemon so a single `cargo build` produces a `wipe` binary with
the UI inside it.
```
