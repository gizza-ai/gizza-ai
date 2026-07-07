# data-format-converter — competitor analysis (2026-07-07)

Built as a new tool. This is the pre-implementation competitor scan required by the
create-next-tool recipe. **All notes paraphrased — no competitor copy, branding, or
trademarks reproduced.**

## Dup / scope decision

The backlog row "data-format-converter — Converts tabular and record data between CSV,
TSV, JSON, and NDJSON formats" overlaps partly with the existing `csv-json-convert`
block, which does CSV⇄JSON (and TSV via a `delimiter=tab` option). Grepping every
`csv*/json*/*convert*` block's `core/src/lib.rs`:

- `csv-json-convert` — CSV⇄JSON only, TSV reachable via a delimiter param. No NDJSON.
- `csv-change-delimiter` — re-delimits CSV/DSV; no JSON/NDJSON.
- `csv-to-yaml`, `xml-to-csv`, `xlsx-to-csv`, `avro-to-json`, etc. — unrelated pairs.
- **No block handles NDJSON as a general interchange format.** `avro-to-json` only
  *emits* NDJSON one-way from Avro; `mock-data-generator`/`structured-data-validator`
  matches were false hits (`jsonlite` parser name, `jsonld` = JSON-LD).

**Decision: genuinely broader — build it.** The differentiators over `csv-json-convert`
are (1) NDJSON / JSONL as a first-class source *and* target (JSON-array⇄NDJSON,
CSV/TSV⇄NDJSON — none of which any block does today) and (2) an explicit 4-format
`from`/`to` matrix with TSV as a named format, rather than a direction+delimiter model.

## Competitors scanned (top real tools)

Search: "online data format converter CSV TSV JSON NDJSON JSONL". Top reachable tools:

1. **TableConvert (csv-to-jsonlines)** — auto-detects delimiter (comma/tab/semicolon/
   pipe); an "object vs array" data-format selector; a "parse JSON in cells" toggle; a
   live grid editor with transpose/dedupe/case ops; client-side, stated ~10 MB file cap;
   sample worked example. JSONLines emphasised as "one JSON object per line". Separate
   pages per direction (CSV→JSONLines, JSON→JSONLines).
2. **ConvertCSV (csv-to-json)** — the richest options set: 7 delimiters (comma, semicolon,
   colon, pipe, tab, caret, space) + auto-detect; first-row-as-headers; skip-N-lines;
   record limit; automatic number/boolean/null inference with a "force numbers" toggle;
   FIVE JSON output shapes (array of objects, keyed/hash JSON, JSON array, column array,
   template); nested objects via `/` in headers; JSONLines/MongoDB one-per-line mode;
   TSV support; save/load settings.
3. **Qodex (csv-to-json)** — paste or upload; TSV handled; "JSON Lines Mode (a.k.a.
   NDJSON), each object on its own line"; minified ("terse") vs formatted output; header
   case options; exclude-empty-fields; nested via slash notation; treats values as
   strings by default (no auto type inference). Copy + download result.

## Table-stakes → where each lands

| table-stake | in/out-of-model | lands in |
| --- | --- | --- |
| CSV, TSV, JSON, NDJSON as source AND target | in-model | `from`/`to` enums (the tool's core) |
| Bidirectional (any format → any format) | in-model | `from`×`to` matrix |
| Auto-detect source format (+ delimiter) | in-model | `from=auto` sniffer (JSON vs NDJSON vs CSV vs TSV) |
| First row = headers toggle | in-model | `headers` bool (default true) |
| Type inference (num/bool/null) with an off switch | in-model | `infer_types` bool (default true); leading-zero-safe |
| Pretty vs minified JSON | in-model | `pretty` bool (JSON-array target) |
| Nested → flat columns (dot/slash notation) | in-model | `flatten` bool (CSV/TSV target, dot-notation) |
| Paste input, copy + download result, live preview | in-model | provided by the shared page framework (auto-run, Copy, text Download) |
| One-click presets per common conversion | in-model | `[[example]]` chips (CSV→JSON, JSON→NDJSON, CSV→NDJSON, NDJSON→CSV) |
| Custom delimiters (semicolon, pipe, colon, caret, space) | in-model, **considered → rejected** | kept the tool a clean 4-named-format matrix; arbitrary delimiters are `csv-json-convert` (delimiter param) / `csv-change-delimiter`'s job. Duplicating them here would blur this tool's identity. Stated as a scope note on the page. |
| Keyed/hash JSON, column-array, custom template output | in-model, **considered → rejected** | niche JSON reshapes, not format conversion; schema bloat. Listed as scope boundary. |
| Un-flatten (dot/slash headers → nested JSON on import) | in-model, **considered → deferred** | reverse of `flatten`; adds real complexity for a less-common need; flatten covers the common tabular-output direction. Noted on the page. |
| Grid/table cell editor, transpose, dedupe, case ops | in-model, **considered → rejected** | that's a spreadsheet, not a converter; other `csv-*` blocks own dedupe/transpose. |
| File upload (vs paste) | platform norm | pure tools use paste; not a gap. |
| ~10 MB size cap | in-model | no artificial cap; bounded by browser memory (stated on page). |

Every table-stake above lands in the descriptor or is an explicitly-recorded
considered/rejected/deferred decision — none dropped silently.

## Resulting descriptor (7 params)

`data` (required) · `from` (auto|csv|tsv|json|ndjson, default auto) ·
`to` (csv|tsv|json|ndjson, default json) · `headers` (bool, default true) ·
`infer_types` (bool, default true) · `pretty` (bool, default false) ·
`flatten` (bool, default false).
