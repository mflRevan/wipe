# wipe design system

The single source of truth for how **wipe** looks and feels — used by **both** the
local desktop UI (`apps/desktop`) and the public website (`apps/web`). If a color,
font, radius, or motion value isn't here, it doesn't ship.

## Principles

1. **Flat, never gradient.** Depth comes from *borders* and *one* subtle elevation
   step — never gradients, glows, or heavy shadows. Gradients read as AI slop.
2. **One palette everywhere.** The tokens below are identical across the app and the
   website. No component invents its own colors.
3. **Earthy + technical.** Warm, papery neutrals (Slate/Cloud/Ivory) with a single
   terracotta accent (Book Cloth), set in the crisp **Geist** grotesque + **Geist
   Mono**. Warm content, precise structure.
4. **Motion is a whisper.** Fast, ease-out, purposeful. Drag-and-drop is the one
   place motion is expressive; everything else is 120–160 ms and subtle. Always
   honor `prefers-reduced-motion`.
5. **Light and dark are equals.** Both are first-class and fully specified.

## 1. Raw palette (exact hex — from the brand swatches)

| Token | Hex | | Token | Hex |
| --- | --- | --- | --- | --- |
| `slate-900` | `#191919` | | `ivory-300` | `#E5E4DF` |
| `slate-800` | `#262625` | | `ivory-200` | `#F0F0EB` |
| `slate-700` | `#40403E` | | `ivory-100` | `#FAFAF7` |
| `cloud-500` | `#666663` | | `book-cloth` | `#CC785C` |
| `cloud-400` | `#91918D` | | `kraft` | `#D4A27F` |
| `cloud-300` | `#BFBFBA` | | `manilla` | `#EBDBBC` |
| `black` | `#000000` | | `focus` | `#61AAF2` |
| `white` | `#FFFFFF` | | `error` | `#BF4D43` |

Two subtle neutrals are derived (flat, allowed) to complete the dark ramp:
`slate-850 #1E1E1D`, `slate-750 #2E2E2C`.

## 2. Semantic tokens (implement as CSS custom properties)

Both apps define these on `:root` (light) and `[data-theme="dark"]` (dark). Component
CSS references **only** `--wp-*` tokens, never raw hex.

```css
/* ---------- Dark (default for the desktop app) ---------- */
[data-theme="dark"] {
  --wp-canvas:        #191919;  /* app background            */
  --wp-surface:       #1E1E1D;  /* board columns             */
  --wp-card:          #262625;  /* cards, inputs, menus      */
  --wp-elevated:      #2E2E2C;  /* hover / dragged / popovers */
  --wp-border:        #302F2D;  /* hairlines                 */
  --wp-border-strong: #40403E;  /* focused / selected edges  */
  --wp-text:          #FAFAF7;  /* primary text              */
  --wp-text-muted:    #91918D;  /* secondary text            */
  --wp-text-subtle:   #666663;  /* ids, timestamps, hints    */
  --wp-accent:        #CC785C;  /* primary actions, active   */
  --wp-accent-hover:  #D8876B;
  --wp-on-accent:     #FAFAF7;  /* text on accent            */
  --wp-focus:         #61AAF2;  /* focus ring only           */
  --wp-error:         #BF4D43;
  --wp-shadow:        0 1px 2px rgba(0,0,0,.40);
  --wp-shadow-lift:   0 10px 30px rgba(0,0,0,.55);
}

/* ---------- Light ---------- */
:root, [data-theme="light"] {
  --wp-canvas:        #FAFAF7;
  --wp-surface:       #F0F0EB;
  --wp-card:          #FFFFFF;
  --wp-elevated:      #FFFFFF;
  --wp-border:        #E5E4DF;
  --wp-border-strong: #D6D5CE;
  --wp-text:          #191919;
  --wp-text-muted:    #666663;
  --wp-text-subtle:   #91918D;
  --wp-accent:        #CC785C;
  --wp-accent-hover:  #B96A4F;
  --wp-on-accent:     #FFFFFF;
  --wp-focus:         #61AAF2;
  --wp-error:         #BF4D43;
  --wp-shadow:        0 1px 2px rgba(25,25,25,.06);
  --wp-shadow-lift:   0 10px 30px rgba(25,25,25,.14);
}
```

Focus states: a **2px** ring in `--wp-focus` (`box-shadow: 0 0 0 2px var(--wp-focus)`),
never a browser default outline. Keyboard focus must always be visible.

## 3. Theme options exposed to the user

- **Appearance:** `Light`, `Dark`, `System` (follows OS). Persisted in localStorage;
  toggle in the header + Settings.
- **Accent:** default **Book Cloth**, with alternates **Kraft** `#D4A27F`,
  **Focus** `#61AAF2`, and **Sage** `#7E9B7A`. Only `--wp-accent`/`-hover`/`-on-accent`
  change; everything else stays neutral. (These are the "extra common color styles".)

## 4. Label / tag colors

A fixed, harmonious set (muted, flat). Each label stores one key; the UI maps it to a
`{bg, text}` pair per theme. Never free-form hex in the UI picker.

`terracotta #CC785C` · `kraft #D4A27F` · `manilla #EBDBBC` · `sky #61AAF2` ·
`clay #BF4D43` · `sage #7E9B7A` · `indigo #6C7BA8` · `plum #9A7AA0` · `slate #666663`

On dark: chip `bg = color @ 22% alpha`, `text = color lightened`, `border = color @ 40%`.
On light: chip `bg = color @ 14% alpha`, `text = color darkened`, `border = color @ 30%`.
No solid-fill chips (too loud), no gradients.

## 5. Typography — Geist

Self-hosted (no CDN), so it works offline and inside the embedded binary.

- **UI:** `Geist` → `@fontsource-variable/geist`. Fallback: `ui-sans-serif, system-ui, sans-serif`.
- **Mono:** `Geist Mono` → `@fontsource-variable/geist-mono`. Used for ticket IDs,
  timestamps, code, and the `.wipe` paths. Fallback: `ui-monospace, SFMono-Regular, Menlo, monospace`.

Weights: 400 (body), 500 (labels, buttons, card titles), 600 (headings, wordmark).

| Role | Size | Weight | Notes |
| --- | --- | --- | --- |
| Wordmark `wipe` | 18px | 600 | mono, tracking `-0.02em`, lowercase |
| Column header | 12px | 600 | uppercase, tracking `0.06em`, `--wp-text-muted` |
| Card title | 14px | 500 | tracking `-0.005em`, max 3 lines then ellipsis |
| Body / drawer | 14px | 400 | line-height 1.55 |
| Meta (id, time) | 12px | 400 | **mono**, `--wp-text-subtle` |
| Micro (badges) | 11px | 500 | |

## 6. Spacing, radius, elevation

- **Spacing scale (px):** 2, 4, 6, 8, 12, 16, 20, 24, 32. Card padding 12; column gap 12.
- **Radius:** `sm 6` (chips, inputs, buttons), `md 8` (cards), `lg 12` (columns, drawer,
  menus), `pill 999` (avatars, counts).
- **Elevation:** resting cards have **no** shadow — just `--wp-border`. `--wp-shadow`
  appears on hover of interactive cards; `--wp-shadow-lift` only on a dragged card and
  the drawer. That's the entire shadow vocabulary.

## 7. Motion

```
--wp-ease: cubic-bezier(0.2, 0, 0, 1);
--wp-fast: 120ms;  --wp-base: 160ms;  --wp-slow: 220ms;
```

- **Hover / press / color:** `--wp-fast`.
- **Drawer / menu open:** `--wp-slow`, slide + fade.
- **Drag & drop** (the expressive moment):
  - On grab: card scales to `1.02`, gains `--wp-shadow-lift`, cursor `grabbing`, source
    slot becomes a dashed `--wp-border-strong` placeholder.
  - Reflow: siblings animate with FLIP at **180ms** (`svelte-dnd-action` `flipDurationMs`).
  - On drop: settle at `--wp-base`; a brief `--wp-accent` edge flash (150ms) confirms.
- `@media (prefers-reduced-motion: reduce)`: disable transforms/FLIP, keep instant.

## 8. Components (behavioral spec)

- **Column:** `--wp-surface`, radius `lg`. Header = uppercase name + count pill +
  `+` add-card. Body scrolls; footer add-card affordance. Optional WIP-limit shown in
  header, turns `--wp-error` when exceeded.
- **Card:** `--wp-card`, radius `md`, `--wp-border`. Layout: mono ID (subtle) + priority
  dot; title; label chips + type chip; footer row = assignee avatars, `💬 n`, `📎 n`.
  Hover: `--wp-elevated` + `--wp-shadow`. Fully keyboard-draggable (dnd a11y).
- **Ticket drawer:** right-side panel (`lg` radius on the inner card, `--wp-shadow-lift`),
  slides in at `--wp-slow`. Inline-edit everything: title, markdown body, **status**
  (list dropdown), **type**, **priority**, **labels** (multi-select colored), **tags**,
  **assignees** (avatar picker), **attachments** (upload + render), **comments** thread.
- **Assignee avatar:** circle with initials, background = a deterministic label color from
  the id. Humans (git identities) show initials; **agents** show a small bot glyph badge.
  Display names are editable (see identities, below). Tooltip shows full id.
- **Buttons:** primary = `--wp-accent` bg / `--wp-on-accent` text; secondary = `--wp-card`
  bg + `--wp-border`; ghost = transparent + hover `--wp-elevated`. Radius `sm`.
- **Inputs / selects:** `--wp-card` bg, `--wp-border`, focus ring `--wp-focus`. Radius `sm`.
- **Toggle (theme):** sun/moon, header-right.

## 9. Media rendering

Attachments render by MIME:

| Type | Rendering |
| --- | --- |
| image, gif, webp, svg | inline `<img>`, max-height 320px, click → full/download |
| audio (mp3, ogg, wav) | native `<audio controls>` |
| video (mp4, webm) | native `<video controls>`, max-height 320px |
| pdf | file card: icon + name + size + **Download** |
| md, txt, csv, log | mono preview (first ~12 lines) + **Download** |
| other | generic file chip + **Download** |

## 10. Assignees, identities & attachments (data rules)

These back the UI features above; the on-disk schema lives in `.wipe/`.

- **Identities** (`.wipe/identities.json`): a list of
  `{ id, display_name, kind: "human" | "agent" }`. Humans are auto-discovered from git
  authors; agents (e.g. `claude`) are added on first use. `display_name` is editable in
  the UI. Ticket `assignees` reference identity `id`s.
- **Attachments** (per ticket): `{ name, path, source, size, mime }` where `path` is
  **repo-relative**.
  - `source: "repo"` — the file is already tracked in the repository; store its existing
    path and reference it. **No copy, no duplication.**
  - `source: "media"` — an external upload; copied to `.wipe/media/<hash8>-<name>` and
    referenced there. On upload we hash the bytes and, if a tracked file already has the
    same git blob hash, we record it as `source: "repo"` instead of copying.
  - **Size limit:** default **50 MB** per attachment (`settings.daemon`/`settings` key
    `max_attachment_mb`), matching git/GitHub's soft warning threshold; larger uploads are
    rejected with a clear message.

---

### Implementation order

1. **Tokens + type first.** Land these CSS variables and the Geist fonts in both apps;
   delete every gradient and ad-hoc color.
2. **Backend capabilities.** `.wipe` schema + daemon endpoints for editing
   labels/tags/status/type/priority, assignees, identities, and media upload/serve.
3. **Rebuild the board UI** against this spec (Trello-grade interactions + the drawer).
4. **Restyle the website** to the same tokens for one coherent brand.
