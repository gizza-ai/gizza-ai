# filter-lines — competitor analysis & differentiation

**Tool:** `gizza-ai/filter-lines` — keep or drop lines matching a substring or
regex.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `grep` / `grep -v` | CLI | The reference, but requires a terminal and remembering flags (`-i`, `-E`, `-v`); not available to non-devs or in a browser. |
| Editor "filter lines" (VS Code, Sublime plugins) | App | Good, but editor-specific and needs the file open in that editor. |
| Online "text line filter" tools | Web | Exist, but many upload your text, often substring-only (no regex), and inconsistent keep/drop semantics. |
| Spreadsheet filters | App | Row-oriented, awkward for free text, no regex. |

## How gizza's tool is better / different

1. **grep for everyone, everywhere.** Substring *or* full regex, keep *or* drop,
   case-sensitive or not — exposed as plain fields/toggles in chat, CLI, and a
   browser page. No flag memorization.
2. **Both match engines.** Literal substring by default (safe, no escaping
   needed) and a real regex mode (`^ERROR`, `\d{3}-\d{4}`, `(warn|error)`) when
   you want it.
3. **Counts, not just output.** Returns total / matched / kept line counts
   alongside the filtered text — useful to confirm a filter did what you expected.
4. **Local + private.** Runs in WASM; your text (logs, data) never leaves the
   device — unlike upload-based web filters.
5. **Clear keep/drop semantics.** `keep` outputs matches; `drop` outputs
   non-matches — no ambiguity.

## Verification

CLI verified: keep `ERROR` over a 5-line log → 2 lines (`ERROR boom`,
`ERROR again`); drop regex `^(INFO|debug)` → 3 lines retained. Page Playwright
covers the same keep-substring and drop-regex paths.

## Scope / honest limitations

- Operates line-by-line (no multi-line regex across newlines) — by design, it's
  a line filter.
- For search-and-replace within lines, use the `find-replace` tool; for CSV row
  filtering, `csv-filter`.

## Possible future enhancements

- Invert-with-context (grep `-A`/`-B`/`-C`).
- Show matched line numbers.
- Highlight the matching span in each kept line.
