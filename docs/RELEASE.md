# Release & deploy

Two independent, automated flows:

**One tagged commit ships everything.** Pushing a `v*` tag fires all three release
workflows in parallel:

| Trigger | Workflow | What it does |
| --- | --- | --- |
| **push to `main`** (touching `apps/web/`) | `deploy-web.yml` | Deploys the website to **Vercel → wipeboard.dev**. |
| **push a `v*` tag** | `release.yml` (cargo-dist) | Builds cross-platform binaries and creates the **GitHub Release** (with the npm installer package attached as an asset). |
| **push a `v*` tag** | `release-plz.yml` | Publishes the crates (`wipe-core`, `wipe-daemon`, `wipe-cli`) to **crates.io**. |
| **push a `v*` tag** | `publish-npm.yml` | Publishes **`@mflrevan/wipe`** to npm via **OIDC trusted publishing** (no token, no 2FA, signed provenance). |

**Releases happen only on version tags.** There are no auto "release PRs" — you bump
the version yourself and push a tag. Pull requests only run CI (`ci.yml`) and a
cargo-dist dry-run (`dist plan`), never a publish.

## One-time setup

**Repository secrets** (Settings → Secrets and variables → Actions):

- `CARGO_REGISTRY_TOKEN` — crates.io API token (✅ set). Your crates.io account also
  needs a **verified email** to publish.
- `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID` — for `deploy-web.yml` (✅ set).
- **npm needs no secret** — it uses OIDC trusted publishing (below).

**npm OIDC trusted publishing:** on npmjs.com → `@mflrevan/wipe` → Settings →
**Trusted Publisher**, point it at repo `mflRevan/wipe`, workflow **`publish-npm.yml`**
(✅ configured). `publish-npm.yml` requests an `id-token` and publishes tokenlessly;
no `NPM_TOKEN` is required.

**Vercel** (wipeboard.dev): a project with **Root Directory = `apps/web`** and the
`wipeboard.dev` domain is set up, with the three secrets above (✅ done).

## The embedded UI

The `wipe` binary serves the board UI from `crates/wipe-daemon/assets/`, which is
**committed** so the UI is embedded across every channel (crates.io, npm, binaries).
Rebuild and commit it whenever the desktop UI changes:

```sh
pwsh scripts/embed-ui.ps1     # or: bash scripts/embed-ui.sh
git add crates/wipe-daemon/assets && git commit -m "chore: rebuild embedded UI"
```

`cargo install wipe-cli` and the prebuilt binaries then all ship the real board.

## Cutting a release

1. Rebuild the embedded UI (above) if the desktop UI changed.
2. **Set the version everywhere with one command** — never hand-edit versions:
   ```sh
   node scripts/set-version.mjs 0.1.2
   ```
   This updates the root `Cargo.toml` (workspace version + internal
   `wipe-core`/`wipe-daemon` dependency versions), the Tauri crate, the frontend
   `package.json`s, and refreshes `Cargo.lock`. crates.io, the npm package, and the
   binaries all derive their version from the root `Cargo.toml`.
3. Commit on `main` and tag — one tag ships everything:
   ```sh
   git commit -am "chore(release): v0.1.2"
   git tag v0.1.2
   git push origin main v0.1.2
   ```
4. Watch the runs: `gh run watch` (or the Actions tab). On success you'll have the
   GitHub Release with installers, `@mflrevan/wipe` on npm, and the crates on crates.io.

## Config files

- `scripts/set-version.mjs` — the single source of version truth; run it to bump.
- `dist-workspace.toml` — cargo-dist config (targets, installers = shell/powershell/npm,
  npm scope/package). Regenerate CI after editing with `dist init && dist generate`.
- `release-plz.toml` — release-plz config; scoped to **crates.io only** (it does not
  create tags or GitHub Releases — cargo-dist owns those).
- `.github/workflows/{ci,release,release-plz,deploy-web}.yml`.
