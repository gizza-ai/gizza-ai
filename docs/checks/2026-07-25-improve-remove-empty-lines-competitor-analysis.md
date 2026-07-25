# remove-empty-lines — competitor analysis (2026-07-25)

Tool: `remove-empty-lines` — "Deletes all blank or whitespace-only lines, compacting the text."
Type: pure (text → text, deterministic, in-browser wasm).

## Scan

Reviewed the top real competitor tools (paraphrased — no copy/branding reproduced):

- **Online Text Tools — Remove All Empty Lines** (onlinetexttools.com/remove-empty-text-lines):
  removes completely blank lines; a "remove lines containing only whitespace" checkbox also
  deletes spaces/tabs-only lines; a "trim whitespace" option cleans leading/trailing spaces on
  the remaining lines.
- **Browserling — Empty Lines** (browserling.com/tools/empty-lines): removes lines that are
  empty or contain only spaces/tabs/newlines. Minimal, single-purpose.
- **usefmtly / thenaom / aifreeforever "collapse" family**: offer a two-way choice —
  *Remove all* (delete every blank/whitespace-only line, no gaps) vs *Collapse* (reduce a run of
  2+ consecutive blank lines down to a single blank line, preserving paragraph separation).
- **CodeShack / TextFixer / CodeBeautify**: browser-based, private (nothing uploaded); blank +
  whitespace-only lines removed; some allow loading text from a URL or uploading a file.

## Table-stakes (each tagged in-model / out-of-model)

| Feature | Decision |
|---|---|
| Remove all blank (empty) lines | in-model — core behavior |
| Also remove whitespace-only lines (spaces/tabs) | in-model — `whitespace_only` checkbox, default ON |
| Trim leading/trailing whitespace on kept lines | in-model — `trim_lines` checkbox, default OFF |
| Remove-all vs Collapse-consecutive-blanks | in-model — `mode` enum (`remove` default, `collapse`) |
| One-click presets | in-model — declarative `[[example]]` chips |
| Private / in-browser, nothing uploaded | in-model — runs as local wasm |
| Copy / download result | in-model — generator provides Copy + Download for text pages |
| Load text from a URL / upload a file | out-of-model (page is paste-only; the `gizza` CLI reads files via a pipe) |
| CRLF↔LF conversion, zero-width / control-char stripping | out-of-model — covered by sibling tools `remove-whitespace` and `remove-control-chars` |
| Line-break / paragraph-joining modes | out-of-model — different tool (join-lines) |

## Dedup note

Distinct from existing blocks:
- `filter-lines` is a generic keep/drop grep (requires a user-supplied regex like `^\s*$`); not a
  dedicated blank-line remover.
- `remove-whitespace` trims/collapses/strips whitespace and can collapse runs of 2+ blank lines to
  a single blank line, but never deletes all blank lines. A standalone "remove all empty lines" is
  a distinct, common tool.

Sources (paraphrased only):
- https://onlinetexttools.com/remove-empty-text-lines
- https://www.browserling.com/tools/empty-lines
- https://usefmtly.com/tools/text-tools/remove-empty-lines/
- https://tools.thenaom.com/en/tools/blankline.html
- https://codeshack.io/remove-empty-lines/
- https://www.textfixer.com/tools/empty-line-remover.php
