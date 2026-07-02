# Release setup (cargo-dist + release-plz)

This document describes the release automation added to the repo and the
manual, one-time steps a maintainer needs to do to make it fully live. None
of this was run automatically -- no tags were pushed, nothing was published,
and the root `Cargo.toml` was **not** modified.

## What was added

| File | Purpose |
| --- | --- |
| `dist-workspace.toml` | cargo-dist configuration (the modern config file, used instead of `[workspace.metadata.dist]` in `Cargo.toml`). |
| `.github/workflows/release.yml` | GitHub Actions workflow that runs `dist` to build/package/release binaries when a `v*` tag is pushed. |
| `release-plz.toml` | release-plz configuration (conventional commits, changelog, workspace mode). |
| `.github/workflows/release-plz.yml` | GitHub Actions workflow that runs release-plz on push to `main` to open "release PRs" and publish crates to crates.io. |

## Why `dist-workspace.toml` instead of editing `Cargo.toml`

The task constraints for this change said not to touch the root
`Cargo.toml`. cargo-dist >= 0.28 supports (and now prefers) a standalone
`dist-workspace.toml` file at the repo root as an alternative to
`[workspace.metadata.dist]`, so that's what's used here. Functionally it is
equivalent.

## Manual follow-up steps

1. **Install cargo-dist locally** and let it validate/regenerate this setup:
   ```sh
   cargo install cargo-dist --locked
   cargo dist init
   ```
   `cargo dist init` will read the existing `dist-workspace.toml`, ask a few
   interactive questions (targets, installers, etc. are already filled in),
   and may adjust the file to match whatever cargo-dist version you have
   installed.

2. **Regenerate the CI workflow from the config** so it's byte-for-byte what
   your installed cargo-dist version expects (action SHAs, step names, and
   internal `dist` flags occasionally change between versions):
   ```sh
   cargo dist generate-ci
   ```
   This will overwrite `.github/workflows/release.yml`. The version
   committed here is a hand-authored approximation of that output (based on
   cargo-dist v0.32.0's real generated workflow) so it is correct in
   structure, but treat the `cargo dist generate-ci` output as the source of
   truth going forward.

3. **Add repository secrets** (Settings -> Secrets and variables -> Actions):
   - `NPM_TOKEN` -- an npm automation token with publish rights, used by the
     `publish-npm` job in `release.yml` to publish the cargo-dist-generated
     `@mflrevan/wipe` npm installer package.
   - `CARGO_REGISTRY_TOKEN` -- a crates.io API token, used by
     `release-plz.yml` to publish `wipe-core` and `wipe-cli` to crates.io.
   - `GITHUB_TOKEN` is provided automatically by Actions; no setup needed.

4. **Cut the first release** once ready:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
   This triggers `release.yml`, which builds the 5 configured targets, opens
   a GitHub Release with all archives/checksums/installers attached, and
   (if `NPM_TOKEN` is set) publishes the npm installer package.

5. **release-plz** runs on every push to `main` and will open/update a
   "Release PR" that bumps versions and updates `CHANGELOG.md` files based
   on Conventional Commits. Merging that PR is what should be treated as
   "cut a release" long-term -- it's expected to also create/push the
   matching `vX.Y.Z` git tag (release-plz does this automatically), which
   in turn triggers `release.yml` above.

## npm distribution

`npm install -g @mflrevan/wipe` is served by **cargo-dist's generated npm
installer** (`dist-workspace.toml`: `installers = [..., "npm"]`,
`npm-scope = "@mflrevan"`, `npm-package = "wipe"`), published automatically by
`release.yml`'s `publish-npm` job on each tagged release. This is the single,
canonical npm path; the earlier hand-rolled `npm/` wrapper was removed to avoid
confusion.

## Embedding the desktop UI into release binaries

The `wipe` binary serves the human board UI from assets embedded at compile time
(`crates/wipe-daemon/assets/`, gitignored). A plain `cargo build` produces a
working binary that falls back to a placeholder page; to ship the **real** board
UI in release binaries, the desktop app must be built and staged **before** the
Rust build.

After running `cargo dist init` / `cargo dist generate-ci`, add a step to the
build job(s) in `release.yml` that runs the embed script before the cargo build,
e.g.:

```yaml
- uses: pnpm/action-setup@v4
  with: { version: 9 }
- uses: actions/setup-node@v4
  with: { node-version: 20, cache: pnpm, cache-dependency-path: apps/desktop/pnpm-lock.yaml }
- name: Build and embed the desktop UI
  run: bash scripts/embed-ui.sh   # builds apps/desktop -> crates/wipe-daemon/assets
```

Locally, `pwsh scripts/embed-ui.ps1` (Windows) or `scripts/embed-ui.sh` (unix)
does the same before `cargo build --release`.

