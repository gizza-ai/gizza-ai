# ndjson-filter — competitor analysis (2026-07-31)

Tool: filter + reshape newline-delimited JSON (NDJSON / JSON Lines) records with a
predicate expression and a field-selection expression. Pure, browser-local, no upload.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **jq** (`jqlang.org` manual + community cheatsheets). The de-facto CLI JSON processor.
   Processes each JSONL line independently. Predicate via `select(.field == value)`;
   comparison ops `== != < <= > >=`; logical `and` / `or` / `not`; nested access via
   `.a.b`; reshape via object construction `{name: .name, email: .email}`. `-s` slurps
   lines into an array; `-c` compacts. Substring via `contains`; regex via `test()`.
   Free/OSS. Weakness for a casual user: full programming language, steep syntax.

2. **jsonutils.org — JSON Filter Tool**. Web tool offering two query modes: jq syntax
   (`.[] | select(.age > 25)`) and JSONPath (`$[?(@.age > 25)]`). Comparison + logical
   `and/or/not`, `select()`, array indexing. Field extraction (`.name`), object
   construction / renaming, `map()`. Output: downloadable filtered results; a visual
   query-builder with templates; sample-data loader; syntax validator; performance metrics.

3. **aidevhub.io — NDJSON/JSONL Viewer**. Paste/upload JSONL. Column-based filtering,
   regex matching, range queries (e.g. timestamp comparisons), live updates. Columns
   auto-detected from object keys → interactive table. Export as **CSV, JSON array, or
   JSONL**. Per-line validation with line numbers + error detail; blank lines skipped;
   non-object lines flagged. Copy result, clear input, sample data, sortable headers,
   Table vs Raw view. Limit = browser memory.

4. **devtooleasy.com / jsonfmt.dev — JSONL Viewers**. Paste/upload, per-line validation
   & formatting, substring search across keys/values, export back to CSV / JSON array /
   JSONL. Positioned as validators + formatters + search.

5. **jf (PyPI) / ndjson.com tooling**. CLI JSONL query engines: per-record predicates,
   field projection, format conversion (JSONL ↔ array ↔ CSV). CLI/Python only.

## Table-stakes (extracted)

| Capability | Competitors | In our model? |
| ---------- | ----------- | ------------- |
| Per-record predicate, comparison ops `== != > >= < <=` | jq, jsonutils, viewer | **yes** |
| Logical `and` / `or` / `not` + parens | jq, jsonutils | **yes** |
| Nested/dotted field access `a.b.c`, array index | all | **yes** |
| Substring `contains` | jq, viewers | **yes** (`contains`/`startswith`/`endswith`) |
| Regex match | jsonutils, viewer | **yes** (`~` / `matches`, `regex` crate) |
| Field selection / projection | all | **yes** (`fields`) |
| Field **rename** / reshape | jq, jsonutils | **yes** (`new=old.path`) |
| Output as JSONL / JSON array / **CSV** | viewers, jf | **yes** (`format=ndjson\|array\|csv`) |
| Skip / report invalid lines with line number | viewers | **yes** (`skip_invalid` + `line N:` errors) |
| Limit output count | jf, viewers (paging) | **yes** (`limit`) |
| Invert / `not` whole predicate | jq (`not`) | **yes** (`invert`) |
| Sample/preset examples | jsonutils, viewers | **yes** (`[[example]]` chips) |
| Copy result / reset | all | **yes** (generator provides both) |

## Decisions — in-model vs out-of-model

**In-model (built):**
- Deterministic predicate mini-language: `path op value` clauses joined by
  `and`/`or`/`not` with `( )` grouping. Ops: `== != > >= < <= contains startswith
  endswith ~/matches`. Numeric compare when both sides are numbers, else string.
  Bare `path` = exists-and-truthy. Missing path treated as absent (see limits).
- Field selection/reshape `fields`: comma-separated `[outname=]dotted.path`; empty =
  keep whole record. Missing path → `null`.
- Output `format`: `ndjson` (default, compact one-per-line), `array` (pretty JSON
  array), `csv` (header from output keys / first-seen union, RFC-4180 quoting).
- `invert`, `limit`, `skip_invalid` toggles. Line-numbered parse errors.
- Example preset chips (filter, reshape, csv-export).

**Out-of-model (considered, not built):**
- Interactive sortable table / live column view — no interactive grid on the page
  surface (single form → single text output). We ship the equivalent as CSV/array export.
- Visual query-builder UI, performance dashboards — chrome, not compute.
- Full jq programmability (pipes, `map`, arithmetic, custom functions) — this is a
  focused filter/reshape tool, not a jq reimplementation; `jq-query` / `jsonata-query`
  blocks already cover general JSON programs. Kept intentionally small and predictable.
- File upload of multi-GB streams — browser-memory bound; input is pasted text.

## Not a duplicate
Distinct from `jq-query`/`jsonata-query` (full query languages, not a focused
predicate+projection filter), `jsonl-deduplicator` (dedup, not filter), `csv-filter`
(CSV rows), `filter-lines` (plain text lines, not JSON records), and `json-*` converters.
