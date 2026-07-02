# wipe desktop

The local, human-facing desktop UI for **wipe** — a git-native task board. A
Trello-style board that talks to the local `wipe-daemon` REST/WebSocket API.

Built with **SvelteKit** (static SPA), **TypeScript**, **Tailwind CSS**,
hand-rolled shadcn-style components, and **svelte-dnd-action** for drag & drop.

## Develop

```bash
pnpm install
pnpm dev        # Vite dev server on http://localhost:5173
```

The UI expects the daemon at `http://localhost:6737`. Start it from any wipe
project with `wipe serve`. When the daemon is unreachable the app degrades
gracefully and shows a "start `wipe serve`" screen.

Configure the API base URL either at build/dev time via the `VITE_WIPE_API`
environment variable, or at runtime in the in-app **Settings** dialog (stored in
`localStorage`, overrides the env var).

```bash
VITE_WIPE_API=http://localhost:6737 pnpm dev
```

## Build (static SPA)

```bash
pnpm build      # -> ./build/index.html  (adapter-static, fallback SPA)
pnpm preview    # serve the production build locally
pnpm check      # svelte-check (type checking)
```

`pnpm build` emits a fully static site under `build/` (`ssr = false`,
`prerender` off, `fallback: index.html`). Those assets are:

- embedded into **`wipe-daemon`** and served by **`wipe serve`**, and
- bundled by the Tauri wrapper below.

## Features

- **Project switcher** — dropdown backed by `GET /api/projects`, with a live
  daemon health indicator (`GET /api/health`).
- **Board view** — lists as columns, tickets as cards; drag cards within and
  between columns. Drops call `POST /api/tickets/{id}/move` optimistically and
  reconcile via WebSocket/refetch.
- **New ticket** — a "+" per column opens a dialog (title, type, priority, body)
  → `POST /api/tickets`.
- **Ticket detail drawer** — full body, metadata, and comment thread with an
  add-comment box (`POST /api/tickets/{id}/comments`).
- **History rewind / time machine** — a commit slider (`GET /api/history`)
  loads read-only historical snapshots (`GET /api/board/at`) with a GitLens-style
  attribution banner and a "Return to now" button. Dragging is disabled.
- **Live updates** — subscribes to `GET /ws`; refetches the board on `changed`.

Key modules: `src/lib/api.ts` (typed client + WS), `src/lib/stores/board.ts`
(board state), `src/lib/components/*` (UI).

## Native app (Tauri v2)

A `src-tauri/` scaffold wraps this UI in a native window titled "wipe". It points
`build.frontendDist` at `../build` and `build.devUrl` at the Vite dev server.

Building the native app requires the Tauri CLI and a native toolchain:

```bash
cargo install tauri-cli --version '^2.0'
# or: pnpm add -D @tauri-apps/cli

pnpm build          # produce ./build first
pnpm tauri build    # native bundle  (NOT run in CI here)
```

> The `src-tauri` crate is intentionally standalone (its own empty `[workspace]`),
> so it does not affect the Rust crates in `../../crates`. Native icons under
> `src-tauri/icons/` are not committed; generate them with
> `pnpm tauri icon <path-to-png>` before a native build.

The same static build is also embedded into `wipe-daemon` and served by
`wipe serve`, so a native shell is optional.
