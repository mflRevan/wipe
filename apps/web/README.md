# wipe — web

The public marketing + documentation website for [wipe](https://github.com/mflRevan/wipe),
a git-native task board for humans and agents.

Built with Vite + React + TypeScript, Tailwind CSS, shadcn/ui-style components,
and react-router-dom. Dark-mode, minimal aesthetic.

## Develop

```sh
pnpm install
pnpm dev        # start the dev server (http://localhost:5173)
```

## Build

```sh
pnpm build      # type-check + production build into dist/
pnpm preview    # serve the production build locally
```

## Deploying under a subpath

The site defaults to serving from `/`. To deploy under a subpath (e.g.
`https://example.com/wipe/`), set `base` in `vite.config.ts`:

```ts
export default defineConfig({ base: "/wipe/", /* ... */ });
```

The router reads `import.meta.env.BASE_URL`, so client-side routing follows the
configured base automatically.

## Structure

- `src/pages/Landing.tsx` — landing page (hero, why, features, quickstart)
- `src/pages/Docs.tsx` — docs layout with sidebar + sections
- `src/pages/docs/data.ts` — docs sections and CLI reference data
- `src/components/CodeBlock.tsx` — reusable copyable code block
- `src/components/CommandCard.tsx` — CLI command card
- `src/components/ui/` — shadcn-style primitives
