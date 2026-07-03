# Contributing to wipe

Thanks for your interest in contributing to `wipe`! This document covers
what you need to build, test, and submit changes.

## Prerequisites

- **Rust** (stable toolchain - see `rust-toolchain.toml` for the pinned
  version). Install via [rustup](https://rustup.rs/).
- **Node.js 20+** and **pnpm** (for the `apps/desktop` and `apps/web`
  frontends).

## Repository layout

- `crates/wipe-core` - the deterministic board data model, serialization,
  and git-native storage logic.
- `crates/wipe-cli` - the `wipe` command-line interface.
- `crates/wipe-daemon` - the local daemon backing the desktop/web UX.
- `apps/desktop` - SvelteKit + shadcn-svelte + Tauri desktop app.
- `apps/web` - supporting web frontend.

## Building

Build the full Rust workspace:

```sh
cargo build --workspace
```

For frontend work, install dependencies and run the relevant app with pnpm
from within `apps/desktop` or `apps/web`.

## Testing

Run the Rust test suite:

```sh
cargo test --workspace
```

CI runs tests with [`cargo nextest`](https://nexte.st/); if you have it
installed locally you can match CI more closely with:

```sh
cargo nextest run --workspace
```

## Formatting

```sh
cargo fmt
```

Please run this before committing - CI checks formatting and will fail on
unformatted code.

## Linting

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

All clippy warnings must be resolved (or explicitly and justifiably
`#[allow]`ed) before a PR can merge.

## The one hard rule: writes go through `wipe-core`

All writes to files under `.wipe/` **must** go through `wipe-core`'s
deterministic serializer. Do not hand-roll JSON serialization for board or
ticket state anywhere else in the codebase (CLI, daemon, or frontends). This
is what keeps `.wipe/` diffable and low-conflict across branches and agents -
bypassing it, even for a "small" field, breaks that guarantee for everyone.

## Commit messages

This project follows [Conventional Commits](https://www.conventionalcommits.org/).
Examples:

```
feat(cli): add wipe ticket move command
fix(core): correct ticket ordering on list merge
docs: update quickstart in README
```

## Branching and pull requests

- Development happens trunk-based on `main`. Please branch off `main` for
  your changes.
- All changes land via pull request - no direct pushes to `main`.
- CI must be green (build, tests, `cargo fmt --check`, `cargo clippy`)
  before a PR is merged.
- Keep PRs focused; a small PR with a clear description is much easier to
  review than a large one that mixes concerns.

## Questions

Open an issue, or reach out to the maintainer at aiman@shabib.net.
