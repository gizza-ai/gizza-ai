# count-line-frequency — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/count-line-frequency` — count how often each line/value occurs
and rank them most→least frequent. Pure-Rust, dependency-free. Pure-text input →
text/data output: chat + CLI + a page.

## What competitors do

- **`sort | uniq -c | sort -rn`** — the classic shell one-liner; powerful and
  local, but requires a terminal and the right pipe order (and `uniq` only collapses
  *adjacent* duplicates, so you must `sort` first — a common gotcha).
- **Online "count duplicates / word frequency" sites** — easy, but the text is
  uploaded and they vary in options.
- **Spreadsheets** (`COUNTIF`, pivot tables) — capable but fiddly for a quick tally.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The text never leaves the device.
2. **One step, correct ranking.** Counts all lines (no need to pre-sort, unlike
   `uniq`) and returns them **most-frequent-first**, with stable first-seen order
   for ties.
3. **Useful options.** `case_sensitive` (group `Apple`/`apple`, keeping the
   first-seen casing) and `trim` (ignore surrounding whitespace); blank lines are
   skipped automatically.
4. **Structured output.** Chat/CLI return each value with its count plus the number
   of distinct values and the total — easy for an LLM or script to consume; the
   page shows a `count<TAB>value` table.

## Honest scope

- **Whole-line counting** (one value per line) — not word/token frequency within a
  line (that's a different tool) and not regex grouping.
- Counts are exact; for extremely large inputs the result list can be long (no
  top-N cap — the caller can slice).

## Tests

7 core unit tests: ranks by frequency with correct distinct/total; ties keep
first-seen order; case-insensitive grouping keeps first-seen casing; trim +
blank-skipping; no-trim distinguishes whitespace; the `count<TAB>value` table
format; and empty input. Plus the block drift-guard schema test. **CLI verified**
end-to-end. **Page** verified with Playwright. `wafer build` instantiates the chat
block.
