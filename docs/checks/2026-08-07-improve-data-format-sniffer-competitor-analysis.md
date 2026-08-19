# data-format-sniffer — competitor analysis (2026-08-07)

Scan run **before** implementing, per `/create-next-tool` step 4. All notes are
**paraphrased observations of behaviour**; no competitor copy, branding, wording,
or trademarks are reproduced or reused anywhere in this tool.

Search: `online tool detect data format CSV TSV JSON XML delimiter detection sniffer`.

## Competitors skimmed

### C1 — ToolsRail "Data File Separator Detector" (browser tool)
- **Input:** upload (csv/tsv/txt/log) or paste, with sample data pre-loaded in the paste box.
- **Options (defaults):** handle quoted fields (on), support multi-line fields (on),
  skip empty lines (on), first row is header (on); delimiter candidate set
  comma / tab / semicolon / pipe / colon / tilde / space; a custom-delimiter add box.
- **Output:** detected delimiter (name + symbol), a confidence percentage derived from
  row consistency, a per-candidate frequency breakdown, a parsed preview of the first
  5 rows, and alerts when column counts are inconsistent.
- **Limits stated:** analysis samples only the first 100 lines; very large files can
  destabilise the browser.
- **FAQ topics:** local processing, what to do when detection is wrong, how to read the
  confidence score, multi-delimiter files, sensitive data, large files, detection vs validation.

### C2 — DuckDB CSV sniffer (reference implementation, engine-side)
- **Dialect detection:** delimiter candidates `,` `|` `;` tab; quote candidates `"` `'` none;
  escape candidates quote-like plus backslash; newline candidates `\r` `\n` `\r\n` and mixed.
  It evaluates the candidate combinations and keeps the one producing the most columns with
  the most consistent per-row column counts.
- **Type detection:** candidates tried most-specific-first — null, boolean, bigint, double,
  time, date, timestamp, varchar — casting values from the first chunk and dropping candidates
  that fail; several common date and timestamp layouts are attempted.
- **Header detection:** the first row is cast against the detected column types; a cast
  mismatch is read as "this row is a header", otherwise generated column names are used.
- **Sampling:** default ~20k rows, configurable.

### C3 — onlinetools.com "Validate CSV" (browser tool)
- **Input:** upload, paste, and a URL query parameter that pre-fills the input.
- **Options (defaults):** delimiter (comma), quote character (double quote), comment
  character, allow comments (on), allow empty lines (on), allow empty values (on),
  allow incomplete data (on), allow leading/trailing spaces (on), error display limit (10).
- **Output:** valid/invalid status plus a list of violations with row numbers and violation type.
- **Presets:** several one-click examples (basic comma, semicolon, quoted fields, multiple
  errors, leading/trailing spaces).
- **Limits:** usage is metered — a free daily allowance with a paid unlimited tier.

## Table stakes → decision

| Table stake (source) | Decision | Where it lands |
| --- | --- | --- |
| Paste input (C1, C3) | **in-model** | required `data` param, multiline page textarea |
| Query-param deep link (C3) | **in-model** | every param is deep-linkable (`?data=…`) — Playwright-tested |
| Delimiter candidate set incl. tilde/colon/space (C1) | **in-model** | built-in candidates `, \t ; \| : ~ space` |
| Custom/extra delimiter (C1) | **in-model** | `extra_delimiters` param |
| Confidence percentage (C1) | **in-model** | `confidence` in report + JSON |
| Per-candidate frequency breakdown (C1) | **in-model** | `delimiter_scores` list in the report and JSON |
| Quote-character detection (C2) | **in-model** | detected (`"`, `'`, or none), not just assumed |
| Multi-line quoted fields (C1) | **in-model** | record-based quote-aware parser, newline inside quotes stays in the field |
| Header detection (C1 assumes, C2 detects) | **in-model** | detected tri-state (`likely`/`unlikely`/`unknown`) via type mismatch, C2's approach |
| Per-column type detection (C2) | **in-model** | `detect_types` (default on): null/boolean/integer/float/date/datetime/string |
| Line-ending detection (C2) | **in-model** | `lf`/`crlf`/`cr`/`mixed`/`none` |
| Sample-size cap (C1 100 lines, C2 ~20k rows) | **in-model** | `sample_lines`, default 100, max 10000 |
| Row preview (C1, 5 rows) | **in-model** | `preview_rows`, default 5, max 50 |
| Ragged / inconsistent column alerts (C1) | **in-model** | notes list the first offending line numbers |
| Comment-line handling (C3) | **in-model** | `comment_prefix` (off by default — a `#` can be data) |
| One-click example presets (C3) | **in-model** | four `[[example]]` chips on the page |
| Machine-readable output | **in-model** (gap: no competitor scanned offers it) | `output=json` |
| Non-CSV format identification: JSON / JSONL / XML / HTML / Markdown table / fixed-width (gap) | **in-model** (differentiator) | all detected; the scanned tools only answer "which delimiter" |
| Parquet / Avro identification (backlog requirement) | **in-model** | magic-byte check, reachable by pasting bytes as base64 or hex (`input_form`) |
| Encoding detection (backlog requirement) | **in-model, with an honest boundary** | BOM sniff + statistical detection over real bytes when `input_form=base64\|hex`; text pasted into a browser is already Unicode, so `input_form=text` reports UTF-8 by construction and says so |
| Validation with per-row error listing (C3) | **considered, rejected** | that is a different tool's job — this repo already ships `csv-structure-validator` / `structured-data-validator`; a sniffer that also validated would duplicate them |
| File upload (C1, C3) | **out-of-model on this page** | pure-compute pages render field inputs only (file inputs belong to the ffmpeg page runtime), and the chat/CLI schema is `Input::None`. Mitigated: paste, or `base64`/`hex` for binary; `detect-file-type` covers whole binary files by magic bytes |
| Convert/export to another delimiter (C1) | **out-of-model** | conversion is other tools' scope (`csv-change-delimiter`, `csv-json-convert`, …); this tool only reports |
| Accounts, metered free tier, paid unlimited (C3) | **out-of-model** | gizza tools are local, free, no account |
| Whole-file streaming of 1 GB+ inputs (C1 warns against it) | **out-of-model** | wasm sandbox memory; a 1 MiB input cap is enforced and stated on the page |

## Notes on scope honesty

- Encoding for `input_form=text` is reported as UTF-8 **by construction**, with a note
  telling the user to paste bytes as base64/hex to detect an original file encoding.
  Faking a detection result for already-decoded text would be dishonest.
- YAML is only reported when a document marker (`---` / `%YAML`) is present; a general
  YAML heuristic collides with colon-delimited data, so the limit is stated on the page
  instead of guessing.
- No competitor copy, layout, CSS, or naming was reused. All page copy, the report
  layout, and the JSON shape are original to this tool.
