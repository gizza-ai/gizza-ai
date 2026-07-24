# duplicate-row-finder — competitor analysis (2026-07-22)

Function scanned: "find / report duplicate rows in a CSV or delimited table online".
One WebSearch; top real competitors skimmed (paraphrased only — no copy/branding reproduced).

## Competitors skimmed

1. **Data Den — CSV Duplicate Checker** (dataden.tech) — browser-local duplicate
   detection, selected-column matching, normalized matching, downloadable reports;
   can export duplicate rows, clean rows, or a summary report.
2. **Ivandt — Remove CSV duplicates** (ivandt.com) — in-browser, private; key-based
   (compare specific columns) OR exact-row matching; normalization: case-insensitive
   and trim spaces. (Primary action is removal, but exposes the same matching model.)
3. **anyjson.in — CSV Duplicate Rows Viewer** — paste or upload CSV, identifies and
   *views* the duplicate rows (read-only viewer, closest analogue to ours).

(Also seen: Datablist merge-assistant, CSV Explorer — heavier editor products, out of
scope for a single paste-in tool.)

## Table-stakes → decision

| Table-stake capability | In/out of model | Where covered |
|---|---|---|
| Exact whole-row duplicate detection | in | `columns` blank → whole-row key |
| Key-column matching (subset of columns) | in | `columns` = header names or 1-based indices |
| Case-insensitive matching | in | `ignore_case` (default on) |
| Trim / collapse whitespace before compare | in | `ignore_whitespace` (default on) |
| Delimiter choice (comma/tab/semicolon/pipe/custom) | in | `delimiter` |
| Header row awareness | in | `header` (default on) |
| Report / summary output | in | `output = report` |
| Export just the duplicate rows (CSV) | in | `output = csv` |
| Structured/JSON output for scripting | in | `output = json` |
| Downloadable result | in | page `format = "text"` → generic Download link |
| Preset examples / one-click samples | in | three `[[example]]` chips |
| Which columns drive the duplication | in (our differentiator) | per-column repeat profile in report + json |
| Line numbers for each duplicate | in (our differentiator) | report + json `lines` |

## Deliberately out-of-model (listed, not built)

- **Return the cleaned / de-duplicated dataset** (competitors' "remove duplicates" /
  "clean rows" export): out of scope by design — this tool is the read-only *reporter*.
  Removal is `blocks/csv-dedupe` (exact) and `blocks/fuzzy-dedupe` (typo-level); the
  `output = csv` mode already exports the offending rows for review.
- **Interactive merge assistant / conflict resolution** (Datablist-style): a stateful
  multi-step UI, not expressible in the single-compute page/CLI model.
- **Fuzzy / typo-level near-duplicate matching**: covered by `blocks/fuzzy-dedupe`
  (Levenshtein similarity). Here matching is exact after the chosen case/whitespace
  normalization — stated explicitly in the page limits.

## UX controls

Enum `output` renders as a `<select>` with friendly `[input.labels]`; booleans render
as checkboxes (defaults reflected); three `[[example]]` preset chips prefill common
scenarios (whole-row dupes, duplicate emails only, export duplicate rows). No competitor
UX control (sliders/color) applies to a text-in/text-out table tool.

## Verdict

Not a duplicate of the removal tools (`csv-dedupe`/`fuzzy-dedupe` rewrite data; this
reports) nor of `find-duplicate-lines` (whole-line counts, not column-aware CSV rows).
All table-stakes covered in-model; the column-driver profile + line numbers exceed the
scanned competitors.
