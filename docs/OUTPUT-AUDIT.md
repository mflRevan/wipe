# Output audit (0.4.0)

How much each command prints, how that scales with the board, and where output
is truncated, capped, or compacted. Written for agent use: every byte a command
prints lands in an agent's context window.

**Regenerate the numbers** (seeds boards of 10 / 100 / 500 tickets, 60% of them
done, each with a ~1 KB body and five ~300-byte comments, then runs each command):

```sh
cargo test -p wipe-cli --release --test stress -- --ignored output_audit --nocapture
# -> target/output-audit.md
```

Tokens are estimated as bytes / 4. Timings are wall-clock per process on a
Windows 11 dev machine (release build), including process start (~7 ms).

## Measurements

| command | 10 tickets: bytes / ~tokens / lines / ms | 100 tickets | 500 tickets |
|---|---|---|---|
| `status` (human) | 460 / 115 / 15 / 8 | 2859 / 714 / 51 / 15 | 13821 / 3455 / 211 / 45 |
| `status --json` | 916 / 229 / 1 / 8 | 6663 / 1665 / 1 / 14 | 32505 / 8126 / 1 / 42 |
| `status --json --all` | 1856 / 464 / 1 / 8 | 16216 / 4054 / 1 / 14 | 80818 / 20204 / 1 / 45 |
| `status --json --full` | 34316 / 8579 / 1 / 8 | 341982 / 85495 / 1 / 16 | 1710182 / 427545 / 1 / 53 |
| `status --json --full --pretty` (= 0.3 `status --json`) | 41910 / 10477 / 565 / 8 | 416626 / 104156 / 5425 / 17 | 2082826 / 520706 / 27025 / 54 |
| `ticket list --list todo --json` | 345 / 86 / 1 / 7 | 3468 / 867 / 1 / 10 | 17508 / 4377 / 1 / 24 |
| `ticket list --exclude-list done --json` | 699 / 174 / 1 / 7 | 7036 / 1759 / 1 / 10 | 35516 / 8879 / 1 / 24 |
| `ticket list --exclude-list done --fields id,title --json` | 275 / 68 / 1 / 7 | 2796 / 699 / 1 / 11 | 14316 / 3579 / 1 / 26 |
| `ticket list --full --json` | 34318 / 8579 / 1 / 7 | 343334 / 85833 / 1 / 12 | 1717534 / 429383 / 1 / 36 |
| `ticket show T-1 --json` | 3433 / 858 / 1 / 18 | same | same |
| `ticket show T-1 --comments-only --json` | 2143 / 535 / 1 / 7 | same | same |
| `ticket show T-1 --comments 1 --no-activity --json` | 1732 / 433 / 1 / 16 | same | same |
| `ticket show T-1` (human) | 2975 / 743 / 20 / 16 | same | same |
| `comment list T-1 --json` | 2073 / 518 / 1 / 7 | same | same |
| `inbox --all --json` (default cap 50) | 13992 / 3498 / 1 / 7 | 13993 / 3498 / 1 / 11 | 13994 / 3498 / 1 / 30 |
| `inbox --all --limit 0 --json` (uncapped) | 17494 / 4373 / 1 / 8 | 167802 / 41950 / 1 / 12 | 840604 / 210151 / 1 / 34 |
| `inbox --all --limit 20 --json` | 5702 / 1425 / 1 / 7 | 5703 / 1425 / 1 / 11 | 5704 / 1426 / 1 / 29 |
| `forum list --json` (3 threads) | 413 / 103 / 1 / 7 | same | same |
| `forum digest` (3 pinned threads) | 1101 / 275 / 16 / 7 | same | same |
| `comment add` receipt `--json` | 62 / 15 / 1 / 17 | same | same |
| `comment add --echo --json` | 3599 / 899 / 1 / 17 | same | same |
| `ticket edit` receipt `--json` | 73 / 18 / 1 / 17 | same | same |
| `ticket move` receipt `--json` | 37 / 9 / 1 / 18 | same | same |

The field report that motivated 0.4 measured 188 KB for `status --json` on an
82-ticket board. On the 100-ticket board here the 0.3 output would be ~417 KB;
0.4's default is 6.7 KB (60x smaller), and asking one list (`ticket list --list
todo`) is 3.5 KB.

## How each command scales

**Grows with the number of open tickets** (a compact row each, ~140-180 bytes;
row size does not grow with a ticket's history): `status`, `ticket list`. The
done list is collapsed to a count in `status` (use `--all` to expand), so a long
project's history does not inflate it. Use `--list`, `--exclude-list`, `--ready`,
`--since`, `--limit`, and `--fields id,title` to cut further.

**Grows with the whole board's content** (opt-in only): `status --full`,
`ticket list --full`, and `inbox --limit 0`. These are the 0.3-style dumps -
avoid them in agent loops.

**Grows with one ticket's history**: `ticket show` (body + every comment +
activity + up to 10 mentioning commits), `comment list`. Trim with
`--comments N`, `--comments-only`, `--no-activity`, `--no-commits`.

**Constant size**: every write's receipt (37-80 bytes; `--echo` for the full
object), `forum digest` (bounded by `--max-bytes`, default 4000), `inbox` (capped
at 50 events by default), `doctor`, `identity whoami`.

## Truncation and compaction rules

| where | rule |
|---|---|
| `--json` everywhere | one compact line; `--pretty` indents (about +20% bytes) |
| write commands | short receipt `{"ok":true,"id":..,...}`; `--echo` returns the full object |
| `status` | done list collapsed to `{count, collapsed:true}`; rows omit empty fields and don't repeat the list id |
| `ticket list` rows | empty labels/assignees/priority/checklist/criteria/blockers omitted; `blocked_by` lists only *open* blockers |
| `inbox` | newest 50 events by default (`--limit N`, `0` = all), `total` reports the uncapped count; each `detail` is whitespace-collapsed and cut at 120 chars + `…` |
| `forum search` | newest 50 by default (`--limit`, `0` = all); human lines show the first non-empty body line, cut at 100 chars |
| `forum show` | full subtree; bound it with `--depth N` (JSON too) |
| `forum digest` | pinned threads only; each thread's share of `--max-bytes` is capped (min 200 bytes) and cut with a `wipe forum show F-n` pointer; replies are one line each (100-char snippets); a hard cap applies overall |
| `ticket show` | never truncates text; `--comments N` keeps the newest N (`comments_total` keeps the count); commits capped at 10 |
| human `status` / `ticket list` | one line per ticket; titles are never cut |
| hints | stderr only (e.g. the `wipe commit` hint), so JSON stdout is always exactly one object |

## Write latency

Every write takes the board's write lock and, in a git repo, checks whether the
board needs committing: ~17 ms per write process on the measurement machine,
flat across board sizes (writes touch one ticket file and, for moves/creates,
`board.json`). Reads of compact views stay under ~45 ms at 500 tickets.
