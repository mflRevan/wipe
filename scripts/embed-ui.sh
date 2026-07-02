#!/usr/bin/env bash
# Build the desktop UI and stage it into the daemon's embed directory so
# `cargo build` bakes the real board UI into the `wipe` binary.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI="$REPO/apps/desktop"
ASSETS="$REPO/crates/wipe-daemon/assets"

if [ ! -f "$UI/package.json" ]; then
  echo "apps/desktop not present yet; nothing to embed." >&2
  exit 0
fi

pushd "$UI" >/dev/null
[ "${1:-}" = "--skip-install" ] || pnpm install
pnpm build
popd >/dev/null

[ -d "$UI/build" ] || { echo "expected SvelteKit static output at $UI/build" >&2; exit 1; }

find "$ASSETS" -mindepth 1 ! -name '.gitkeep' -exec rm -rf {} + 2>/dev/null || true
cp -r "$UI/build/." "$ASSETS/"
echo "Embedded desktop UI into $ASSETS"
