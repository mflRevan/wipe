#!/usr/bin/env bash
# Agent-to-agent supervision harness (bash mirror of run.ps1).
# Supervisor seeds a wipe board with a spec ticket; a subordinate opencode agent
# discovers and completes it through the wipe CLI; the supervisor verifies.
set -euo pipefail

MODEL="${WIPE_SUBAGENT_MODEL:-opencode/deepseek-v4-flash-free}"
TIMEOUT="${TIMEOUT_SEC:-240}"
KEEP="${KEEP:-0}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
WIPE="$REPO/target/debug/wipe"
[ -x "$WIPE" ] || WIPE="$REPO/target/debug/wipe.exe"
[ -x "$WIPE" ] || { echo "wipe binary not found; run 'cargo build' first" >&2; exit 1; }
WIPE_DIR="$(dirname "$WIPE")"

WORK="${WORK:-$(mktemp -d "${TMPDIR:-/tmp}/wipe-supervision-XXXXXX")}"
echo "==> work dir: $WORK"
echo "==> model:    $MODEL"

(
  cd "$WORK"
  git init -q
  git config user.email "supervisor@wipe.dev"
  git config user.name "Supervisor"
  "$WIPE" init . --name "Calc Service" >/dev/null
  "$WIPE" ticket create --title "Implement add(a, b) in calc.py" --type feature --list todo \
    --body "Create a file calc.py at the repository root that defines a function add(a, b) returning a + b.

Acceptance criteria:
- calc.py exists at the repo root.
- It defines def add(a, b) returning a + b.

When finished: add a comment on this ticket summarizing what you did, then move this ticket to the done list." >/dev/null
  git add -A && git commit -q -m "chore: seed spec board for subordinate agent"
)

read -r -d '' PROMPT <<'EOF' || true
You are a software worker agent. Your tasks are tracked in a CLI task board called `wipe`.
Coordinate ALL task state through the `wipe` command; never read or edit files under .wipe/ directly.

Do this now, in order:
1. Run: wipe skill        (learn the CLI)
2. Run: wipe ticket list --list todo --json     (find your assigned work)
3. Run: wipe ticket show T-1 --json             (read the spec)
4. Implement exactly what ticket T-1 asks, in this repository.
5. Run: wipe comment add T-1 --body "<one-line summary of what you did>"
6. Run: wipe ticket move T-1 --to done

Keep going until T-1 is in the done list. Be concise.
EOF

echo "==> launching subordinate agent (timeout ${TIMEOUT}s)..."
export PATH="$WIPE_DIR:$PATH"
timeout "${TIMEOUT}s" opencode run --dir "$WORK" --model "$MODEL" --auto "$PROMPT" \
  >"$WORK/agent-output.log" 2>&1 || echo "(subordinate exited non-zero or timed out)"

# --- verification ---
FILE_EXISTS=false; DEFINES_ADD=false
[ -f "$WORK/calc.py" ] && FILE_EXISTS=true
$FILE_EXISTS && grep -Eq 'def[[:space:]]+add[[:space:]]*\(' "$WORK/calc.py" && DEFINES_ADD=true
LIST=$("$WIPE" -C "$WORK" ticket show T-1 --json | python3 -c 'import sys,json;print(json.load(sys.stdin).get("list",""))' 2>/dev/null || echo "")
COMMENTS=$("$WIPE" -C "$WORK" ticket show T-1 --json | python3 -c 'import sys,json;print(len(json.load(sys.stdin).get("comments",[])))' 2>/dev/null || echo 0)
IN_DONE=false; [ "$LIST" = "done" ] && IN_DONE=true
HAS_COMMENT=false; [ "$COMMENTS" -ge 1 ] 2>/dev/null && HAS_COMMENT=true

PASS=false
if $FILE_EXISTS && $DEFINES_ADD && $IN_DONE && $HAS_COMMENT; then PASS=true; fi

cat > "$WORK/result.json" <<JSON
{"model":"$MODEL","file_exists":$FILE_EXISTS,"defines_add":$DEFINES_ADD,"ticket_list":"$LIST","in_done":$IN_DONE,"has_comment":$HAS_COMMENT,"pass":$PASS}
JSON
echo "==== supervision result ===="; cat "$WORK/result.json"; echo; echo "============================"
$PASS && echo "PASS" || echo "FAIL"
[ "$KEEP" = "1" ] || rm -rf "$WORK"
$PASS || exit 1
