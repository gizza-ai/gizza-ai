# find-duplicate-lines — competitor analysis & differentiation

**Tool:** `gizza-ai/find-duplicate-lines` — list the lines that appear more than
once, with their counts.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `sort \| uniq -d` / `uniq -c` | CLI | Works, but `uniq` only collapses *adjacent* duplicates (you must `sort` first, which loses order), and the flag combo for "duplicates with counts" is non-obvious. |
| Spreadsheet `COUNTIF` + conditional formatting | App | Multi-step setup; awkward for free text; no case/trim toggle. |
| Online "find duplicate lines" tools | Web | Common, but many upload your text, are inconsistent about counts, and rarely offer case-insensitive / trim options. |
| Editor plugins | App | Editor-specific; need the file open there. |

## How gizza's tool is better / different

1. **Counts, sorted.** Returns each repeated line *with its count*, most-frequent
   first — not just "these are dups". `uniq` needs `sort | uniq -c | sort -rn`
   gymnastics to match this.
2. **Order-independent.** Counts across the whole text regardless of where the
   duplicates sit (no pre-`sort` required, so the displayed line keeps its
   first-seen form).
3. **Case + whitespace toggles.** Optional case-insensitive and trim-whitespace
   matching, so `Foo`/`foo` or `  x`/`x` can be treated as the same line.
4. **Summary stats.** Also reports total and unique line counts.
5. **Local + three surfaces.** Chat, CLI (`gizza tool find-duplicate-lines`), and
   a zero-upload page — one Rust core. Data never leaves the device.

## Verification

CLI verified on `apple/banana/apple/Apple/cherry/banana/apple`: case-sensitive by
default → `apple ×3`, `banana ×2` (the capitalized `Apple` stays separate), with
`total_lines=7`, `unique_lines=4`. Page Playwright covers counts and the
ignore-case toggle.

## Scope / honest limitations

- Reports duplicates, doesn't remove them — pair with a dedupe tool to clean.
- Whole-line comparison (not per-field) — for CSV key-column dedupe use
  `csv-dedupe`.

## Possible future enhancements

- Show the line numbers where each duplicate occurs.
- Optional minimum-count threshold.
- Output just the unique lines, or just the duplicated ones, on request.
