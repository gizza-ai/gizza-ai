# head-lines — competitor analysis (2026-06-23)

Tool: **Head — First N Lines** (`blocks/head-lines`). Outputs the first N lines
of text (the `head -n` operation), with optional leading-line skipping and
per-line numbering. Pure-Rust, runs on all three surfaces (chat skill, CLI,
standalone page).

## Surfaces verified

- **Chat block**: `wafer build` validates `target/block.wasm` (293 KiB). Drift-guard
  unit test (`schema_json_matches_authored_chat_schema`) passes — LLM-facing schema
  is locked.
- **CLI**: `gizza tool head-lines …` verified:
  - omitted `count` → first 10 lines (the conventional `head` default);
  - `count=3` → first 3 lines;
  - `count=2 skip=1 number=true` → `2\tx\n3\ty` (skip a header, number from the
    original line index).
- **Page**: Playwright (`tool-page-head-lines.spec.ts`, 2 specs) — first-N and
  skip+number paths both pass headless.

## Top competitors surveyed

Generic "first N lines / head" text utilities are a common micro-tool. Representative
classes of competitor:

1. **Unix `head` (coreutils)** — the canonical reference. `head -n N`, `head -c N`
   (bytes), negative `-n -N` (all but last N), `head -q`/`-v` (multi-file
   quiet/verbose headers). Single-file, no byte mode here.
2. **Online "get first N lines" / "extract top lines" web utilities** (TextMechanic-
   style line tools, browserling-style dev tools, "head of file" pastebins). Typical
   feature set: choose N, optionally skip leading lines, optionally show line numbers,
   trim/keep blank lines.
3. **`sed`/`awk` one-liner generators** (`sed -n '1,Np'`, `awk 'NR<=N'`). Same core
   capability framed as a command generator.
4. **Spreadsheet/CSV "first N rows" previewers** — head over CSV rows, often with a
   "skip header" toggle.
5. **Log-viewer "head" panes** — first N lines of a pasted log, sometimes with line
   numbers.

## Capability diff (us vs. the field)

| Capability | Competitors | head-lines | Notes |
|---|---|---|---|
| Keep first N lines | all | ✅ `count` (default 10) | core feature |
| Default N = 10 | coreutils, most | ✅ | omitted `count` → 10 on every surface |
| Skip leading lines (header) | many web tools | ✅ `skip` | like `tail -n +K`; good for CSV headers |
| Number each line | many web tools | ✅ `number` | `cat -n`-style 1-based, counts from original index |
| Preserve CRLF / trailing newline | varies (often lost) | ✅ | round-trips file structure faithfully |
| Bound on huge N | rare | ✅ `MAX_COUNT` 1,000,000 | guards absurd input |
| Privacy / local-only | no (server tools) | ✅ WASM in-browser | nothing uploaded |

## Gaps considered and decisions

- **Byte mode (`head -c N`)**: out of scope for a *line* tool; the sibling backlog
  would address byte truncation separately. Not built — keeps the tool focused and
  avoids overlap.
- **Negative N (all-but-last-N lines)**: that is the inverse/`tail`-family operation,
  a distinct tool, not a head variant. Not built.
- **Multi-file headers (`-q`/`-v`)**: the page/CLI take a single text blob; multi-file
  is not part of the input model. Not applicable.
- **Trim/drop blank lines**: already covered by the existing `filter-lines` /
  remove-blank-lines family — out of scope here to avoid duplication.

## Conclusion

`head-lines` matches the in-model feature set of the competitor field (choose N with a
sensible default, skip a header, number lines) and adds faithful CRLF/trailing-newline
preservation plus fully-local privacy. The remaining competitor features (byte mode,
negative N, multi-file) are distinct tools rather than head capabilities and are
deliberately left out. No copy, branding, or trademarks were copied from any competitor.
