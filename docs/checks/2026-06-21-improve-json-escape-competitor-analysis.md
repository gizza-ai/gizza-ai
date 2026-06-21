# json-escape — competitor analysis & differentiation

**Tool:** `gizza-ai/json-escape` — escape or unescape special characters in a
string for safe use inside JSON.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| freeformatter / jsonformatter "JSON escape" | Web | Common, but most **upload your text**, are ad-heavy, and vary on control-char / unicode handling. |
| `jq -Rs .` | CLI | Works for escape, but a terminal incantation; unescape is a separate trick. |
| Hand-rolled `replace()` | DIY | Almost always wrong — people forget backslashes, control chars, or `\uXXXX`. |
| Editor macros | App | Editor-specific, fiddly. |

## How gizza's tool is better / different

1. **Local — text never uploaded.** Runs in WASM (chat SW + CLI + page).
2. **Spec-correct both ways.** Escaping/unescaping go through a real JSON codec
   (serde_json), so quotes, backslashes, newlines/tabs, and other control chars
   become `\uXXXX` exactly as a parser expects — and unescape is the true inverse.
3. **One tool, both directions** with a mode toggle, plus an optional
   **wrap-in-quotes** for escape so you get a ready-to-paste `"…"` literal.
4. **Forgiving unescape** — accepts input with or without the surrounding quotes.
5. **Three surfaces, one Rust core.**

## Verification

Seven core unit tests: escapes quotes/newlines/tabs, control chars → ``,
wrap-in-quotes, full round-trip (incl. an emoji), unescape with/without quotes,
and rejection of invalid escapes (`\q`). **End-to-end CLI**: escape of
`He said "hi"\nbye` → `He said \"hi\"\nbye`; unescape of `a\tb` → a real tab.
Page Playwright covers escape + unescape.

## Scope / honest limitations

- Operates on a single string value (not whole-document re-encoding — that's
  `json-beautify`). Unescape expects valid JSON string escaping and reports
  errors otherwise.

## Possible future enhancements

- ASCII-only escape mode (`\uXXXX` for all non-ASCII).
- Escape for other targets (CSV, SQL, shell) as a sibling family.
