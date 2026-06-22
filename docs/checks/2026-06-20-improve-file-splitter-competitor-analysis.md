# file-splitter — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/file-splitter` — split a large text/CSV file into pieces
(equal parts, fixed line-count, or fixed byte-size), returned as a ZIP. Chat +
CLI (no page: a ZIP output fits neither the pure-text nor the ffmpeg media page
shape — the F3 no-page file-input pattern, like extract-tar).

## What competitors do

- **`split` / `csplit` (coreutils)** — the reference: `split -n`, `-l`, `-b`.
  Powerful but a shell-only CLI; `-b` cuts mid-line, and it has no CSV-header
  awareness.
- **Online CSV/text splitters** (split-csv.com, csvsplitter, aspose, online
  splitters) — upload a file, get split files. Strengths: CSV header handling on
  some. Weaknesses: the file is **uploaded** (privacy + size caps), many are
  Windows-only desktop apps, and free tiers cap rows.
- **Spreadsheet manual splitting** — copy/paste ranges; tedious and error-prone
  for big files.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm, so it runs
   in the chat Service Worker and headless via the CLI. The file never leaves the
   device.
2. **Three split modes in one tool.** `parts` (N equal parts), `lines` (N lines
   each), `bytes` (≤N bytes each) — covering both "I want exactly K files" and "I
   want files no bigger than X".
3. **Never cuts a line.** Unlike `split -b`, byte-mode splits on line boundaries,
   so every piece is made of whole lines (valid CSV/JSONL rows). Pieces reassemble
   byte-for-byte into the original (verified by a round-trip test).
4. **CSV-header aware.** `csv_header=true` repeats the header row at the top of
   every piece, so each output is independently loadable as CSV — a step most
   `split`-style tools don't do.
5. **One tidy ZIP** with `name-part-001.ext` pieces (original extension kept), so
   the output is a single downloadable file and a chainable `ref`.
6. **Even distribution + guards.** `parts` distributes remainder lines evenly;
   errors on more parts than lines, zero count, empty/header-only input, and caps
   the piece count (10k) to avoid runaway splits.

## Honest scope

- Operates on UTF-8 **text** (CSV/TSV/JSONL/logs/plain text). Binary files aren't
  a target (use a byte-exact splitter for those).
- `bytes` mode measures piece size in bytes of whole lines; a single line longer
  than the budget becomes its own (over-budget) piece by design.

## Tests

9 core unit tests: parts even distribution (5 lines→3+2), parts-more-than-lines
error, lines chunking (incl. preserving a final newline-less line), bytes
line-boundary splitting, an over-long line forming its own piece, CSV header
repeated in each piece, empty/zero/header-only errors, a **round-trip** assertion
(pieces `.concat()` == original), and mode parsing. Plus the block drift-guard
schema test. CLI verified over the wire on a real CSV (`airtravel.csv`) split
into 3 parts with `csv_header=true` → a ZIP of 3 `.csv` pieces, each starting
with the `"Month","1958",…` header.
