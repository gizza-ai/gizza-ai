# list-converter — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/list-converter` — reformat a list between comma / newline /
bulleted / numbered / quoted / space forms, with optional sort and dedupe. Chat +
CLI + page (pure-string, no deps).

## What competitors do

- **Online list tools** (textmechanic, convert.town, delim.co, "comma separator"
  sites) — paste a list, pick output. Strengths: many micro-tools. Weaknesses:
  usually one transform per page (separate pages for "add commas", "add quotes",
  "sort", "dedupe"), some upload the text, and ad-heavy.
- **Spreadsheets / editors** — multi-cursor + transpose tricks; fiddly and manual.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: page
   in-browser, CLI headless, chat Service Worker. The list never leaves the
   device.
2. **One tool, many transforms.** Comma, newline, bulleted (`- a`), numbered
   (`1. a`), quoted (`"a", "b"` for code arrays, with proper escaping), and
   space — plus **sort** and **dedupe** in the same pass — instead of chaining
   five separate single-purpose pages.
3. **Smart input parsing.** `auto` separator detects newlines → commas →
   semicolons, so pasting a column or a CSV row both Just Work; or force the
   split. Items are trimmed and blanks dropped.
4. **Code-friendly quoted output** with backslash/quote escaping, ideal for
   turning a pasted column into a JS/Python array literal.
5. **Three surfaces + deep-links.**

## Honest scope

- Sort is case-insensitive lexicographic (not natural/numeric ordering).
- Dedupe is exact-match (case-sensitive), keeping the first occurrence.
- Output joins are fixed (`, ` for comma, `\n` for lines) — no custom delimiter
  yet (could be a future param).

## Tests

8 core unit tests: comma→newline, newline→comma, bulleted + numbered, quoted with
escaping, sort+dedupe (stable case-insensitive order verified), trim/drop-empties,
space split, and error/parse cases. Plus the block drift-guard schema test. CLI +
Playwright (comma→numbered via fill; sort+dedupe via deep-link) verified — see
commit.
