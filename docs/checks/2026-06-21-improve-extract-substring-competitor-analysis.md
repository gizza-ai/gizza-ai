# extract-substring — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-substring` — pull out a portion of text by start/end
index or everything between two delimiters.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Spreadsheet `MID` / `LEFT` / `RIGHT` | App | 1-based, no negative indexing, no "between delimiters", clunky for ad-hoc text. |
| `cut`, `awk`, `sed` | CLI | Powerful but unfriendly syntax; byte/field oriented; "between two strings" needs an awk/sed incantation people look up every time. |
| Python slicing / `str.split` | Library | Need a REPL; great but not paste-and-go. |
| Online substring tools | Web | Usually index-only, 1-based, no negatives, no delimiter mode, and often upload text. |

## How gizza's tool is better / different

1. **Two extraction modes in one tool.** Character **index** (with Python-style
   negatives and an optional end) *and* **between two delimiters** (returning
   *all* non-overlapping matches, not just the first).
2. **Unicode-correct.** Index mode works on *characters*, not bytes, so accents
   and emoji don't corrupt the slice or panic.
3. **Forgiving.** Out-of-range indices clamp instead of erroring; `start ≥ end`
   yields empty rather than crashing.
4. **All matches between delimiters.** `[` / `]` over `a[1]b[2]c[3]` returns
   `1, 2, 3` — handy for pulling every tagged/bracketed value at once.
5. **Local + three surfaces.** Chat, CLI (`gizza tool extract-substring`), and a
   zero-upload page — one Rust core.

## Verification

CLI verified both modes: index `start=-5` on "hello world" → `world`; delimiters
`[`/`]` on "a[1]b[2]c[3]" → `["1","2","3"]`. Page Playwright covers the same two
modes (negative-index slice and all-matches between delimiters).

## Scope / honest limitations

- Delimiters mode requires both delimiters and is literal (not regex) — by
  design, to stay predictable. Regex extraction is covered by the `jq-query` /
  `find-replace` family.
- Index mode is half-open `[start, end)` like most languages.

## Possible future enhancements

- Optional "first match only" for delimiters mode.
- Regex capture-group extraction as a third mode.
- Inclusive/exclusive delimiter toggle (keep the markers in the result).
