#!/usr/bin/env bash
# Provision a realistic demo wipe board for manual UI / daemon testing.
# Usage: tests/mock-projects/seed.sh [dir] [--serve]
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WIPE="$REPO/target/debug/wipe"; [ -x "$WIPE" ] || WIPE="$REPO/target/debug/wipe.exe"
[ -x "$WIPE" ] || { echo "build first: cargo build" >&2; exit 1; }

DIR="${1:-$REPO/tests/sandbox/demo}"
SERVE=0; [ "${2:-}" = "--serve" ] && SERVE=1
rm -rf "$DIR"; mkdir -p "$DIR"; cd "$DIR"

git init -q; git config user.email "demo@wipe.dev"; git config user.name "Demo"
"$WIPE" init . --name "Payments Service" >/dev/null
"$WIPE" label create backend --color "#3b82f6" >/dev/null
"$WIPE" label create urgent  --color "#ef4444" >/dev/null
"$WIPE" ticket create --title "Design ledger schema"          --priority high --list backlog >/dev/null
"$WIPE" ticket create --title "Implement /charge endpoint" --priority high --list todo --label backend >/dev/null
"$WIPE" ticket create --title "Add idempotency keys"       --list todo --label backend >/dev/null
"$WIPE" ticket create --title "Fix rounding in refunds"        --priority urgent --list in-progress --label urgent --label backend >/dev/null
"$WIPE" ticket create --title "Set up CI"                    --list done >/dev/null
git add -A && git commit -q -m "chore: seed initial board"
"$WIPE" comment add T-4 --body "Repro: refund of 0.10 loses a cent due to float math." >/dev/null
"$WIPE" ticket move T-2 --to in-progress >/dev/null
git add -A && git commit -q -m "feat: start /charge endpoint, note refund bug"

echo "Seeded demo board at: $DIR"
"$WIPE" -C "$DIR" status
[ "$SERVE" = "1" ] && exec "$WIPE" -C "$DIR" serve
