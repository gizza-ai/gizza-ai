# json-dedupe-array — competitor analysis (2026-08-18)

Scan run **before** implementation so the descriptor could ship the table-stakes from day one.
All findings are **paraphrased**; no competitor copy, branding, or trademarks were reused.

## Competitors reviewed

| # | Tool | URL | Reachable |
|---|------|-----|-----------|
| 1 | JSON Duplicates Finder (jsonutils.org) | https://jsonutils.org/json-duplicates-finder.html | yes |
| 2 | JSON Duplicate Remover (toolsvana.com) | https://toolsvana.com/tool/json-duplicate-remover | yes |
| 3 | JSON Deduplicate (toolscandy.com) | https://toolscandy.com/tools/json-deduplicate | yes |

A fourth candidate (thetexttool.com "Dedupe JSON Array") returned no readable option/FAQ content when
fetched, so it was replaced by toolscandy rather than counted as a profile.

## What they ship

**1. jsonutils.org — JSON Duplicates Finder**
- Comparison criteria: whole object, chosen keys, or primitive values.
- Deep vs shallow toggle (deep recurses into nested objects/arrays; shallow compares only immediate
  properties).
- Keep first or last occurrence.
- Highlight-only mode (mark duplicates without removing them).
- Outputs: cleaned array, duplicates-only export, and duplicate counts / data-quality statistics.
- UX: "load sample" button, paste or drag-and-drop input, one action button, progress for big inputs,
  browser-local processing.
- Stated limits: comfortable into the thousands of records; beyond ~10,000 items it suggests simpler
  comparison criteria or chunking.
- FAQ themes: deep vs shallow, first vs last, per-field comparison, dataset size, reviewing duplicates.

**2. toolsvana.com — JSON Duplicate Remover**
- Two radio modes: full-object exact match (default) or compare by specific keys.
- Keys entered comma-separated; nested fields via dot notation (`user.profile.email`).
- Keeps the first occurrence; preserves original array order.
- Statistics summary: duplicates found + unique objects remaining, side-by-side input/output.
- Rejects non-array input; JSON validation with an explicit error message.
- UX: paste / drag-drop / file upload / sample data, "Remove duplicates", "Clear all", copy-to-clipboard.
- No case-sensitivity control; no stated hard limit (says millions of records get slow).

**3. toolscandy.com — JSON Deduplicate**
- Removes duplicate object *keys* (keep last by default, or first) — a different operation from
  array-element dedup.
- Optional "dedupe array values" toggle plus a "recursive" toggle for nested structures.
- Copy + download-as-.json, fullscreen, sample JSON, duplicate count.
- Browser-local; no documented limits or error behavior.

## Gap table → what we built

| Table stake | Verdict | Where it landed |
|---|---|---|
| Whole-element (deep) equality | in-model | default mode: `keys` blank → whole-element structural compare |
| Compare by chosen key fields | in-model | `keys` (comma-separated) |
| Nested fields via dot notation | in-model | `keys` accepts `user.profile.email`; numeric segment indexes an array |
| Keep first / keep last | in-model | `keep` = `first` (default) / `last` |
| Preserve original order | in-model | always — output order follows the input |
| Reject non-array input with a clear message | in-model | error names what was found and points at `root` |
| Duplicates-only export | in-model | `output = duplicates` |
| Counts / duplicate statistics | in-model | `output = report` (total / unique / removed + per-group indexes) |
| Highlight-only (don't remove) | in-model, reshaped | `output = duplicates` / `report` both leave the input untouched and show what *would* go |
| Sample data / preset buttons | in-model | three `[[example]]` preset chips on the page |
| Copy result / download / reset | in-model | provided by the shared page runtime for `format = "text"` |
| Pretty vs minified output | in-model | `indent` 0–8 (0 minifies) |
| Case-insensitive matching (none of the three ship it) | in-model, ahead | `ignore_case` |
| Array nested inside a wrapper object (none ship it) | in-model, ahead | `root` dot-path; the wrapper is preserved in `unique` output |
| Key-order-insensitive equality (none document it) | in-model, ahead | canonical compare ignores key order; output preserves it |
| Stated size limit | in-model | hard cap of 200,000 elements with a named error, stated on the page |

### Considered, rejected

- **Deep vs shallow toggle** (jsonutils). Shallow comparison — treating two elements as equal when
  their immediate scalar properties match while nested objects differ — silently drops records that
  are not actually duplicates. Deep structural equality is the safe default, and the `keys` mode
  already covers "compare only the fields I care about" precisely and predictably. A shallow toggle
  would add a footgun, not a capability.
- **Recursive dedup of every nested array** (toolscandy). That is a document-wide transform, not
  "de-duplicate this array": it conflicts with key-based dedup, with the duplicate/report outputs,
  and with index-based reporting. Kept out to preserve a single, explainable operation.
- **Duplicate object-KEY removal** (toolscandy). Different operation on a different unit (keys, not
  elements), and `json-repair` already resolves duplicate keys in malformed JSON.

### Out of model (browser-local wasm, no account, no server)

- Drag-and-drop / file upload of a `.json` file into the array field — the shared page runtime's file
  input is for media-style blocks; text tools take a paste field. Listed, not built.
- Side-by-side input/output diff panes and progress bars for very large inputs — shared page-runtime
  layout work, not a block capability.
- Saved history, accounts, and API access — all require a backend.

## Family placement (why this is not a duplicate)

- `jsonl-deduplicator` de-duplicates **NDJSON/JSONL**: one JSON value per line, whole-*line* text
  equality by default. A pasted JSON array is a single line to it, so it cannot do this job.
- `sort-json-array` orders elements, `json-array-pluck-field` projects one field (its `unique` flag
  de-duplicates the *plucked scalars*, not the elements), `csv-dedupe` is CSV rows,
  `list-dedupe-merge` is plain-text lists, `json-sort` reorders object keys.
