# Agent-to-agent supervision harness

wipe is explicitly built to be a **communication bottleneck for spec-driven
development** — between humans and agents, and between agents. This harness
proves that story end-to-end: a **supervisor** encodes work as wipe tickets, a
**subordinate** agent (running in a *different* harness — [opencode](https://opencode.ai)
with a DeepSeek model) discovers the work through the `wipe` CLI, does it, and
reports back by moving tickets and adding comments. The supervisor then verifies
the board state.

This is intentionally cross-harness: the subordinate only ever talks to wipe
through the CLI and `wipe skill`, never by reading `.wipe/` directly — exactly how
a real foreign agent would.

## What it exercises

- CLI ergonomics and discoverability for an unfamiliar agent (`wipe skill`, `--help`)
- The `--json` contract and stable exit codes
- The spec-driven loop: assigned ticket → work → comment → move to done
- Performance and robustness of the CLI under real agent use

## Requirements

- The `wipe` binary (built via `cargo build`; the script finds `target/debug/wipe`).
- `git` on PATH.
- [opencode](https://opencode.ai) installed and authenticated with a provider that
  exposes the chosen model. By default the harness uses `opencode/deepseek-v4-flash-free`
  (OpenCode Zen free tier). Override with `-Model` / `$WIPE_SUBAGENT_MODEL`.

## Run it

PowerShell (Windows):

```powershell
pwsh tests/agent-supervision/run.ps1
# options:
pwsh tests/agent-supervision/run.ps1 -Model "opencode/deepseek-v4-flash" -TimeoutSec 300 -Keep
```

Bash (Linux/macOS):

```bash
tests/agent-supervision/run.sh
```

The script provisions a throwaway git+wipe project, seeds a spec ticket, launches
the subordinate agent, then verifies:

1. the deliverable file was created,
2. the ticket was moved to **done**,
3. the subordinate left a comment.

It prints `PASS`/`FAIL` and writes a machine-readable summary to
`result.json` in the work directory. Because it drives a real LLM the exact
transcript varies run to run; the *acceptance checks* are what must hold.

## Files

- `run.ps1` / `run.sh` — the harness (provision → run subordinate → verify).
- `scenario.md` — the human-readable spec the supervisor encodes into tickets.
