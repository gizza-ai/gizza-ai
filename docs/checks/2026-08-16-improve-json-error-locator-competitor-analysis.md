# json-error-locator — competitor analysis (2026-08-16)

Scan run BEFORE implementation, per `/create-next-tool` step 3 / `/improve-tool` Phase 2.
All findings are paraphrased observations of publicly visible behaviour. No competitor copy,
branding, or trademark text is reproduced or reused anywhere in the tool.

## Search

One WebSearch: *"JSON error locator validator show line column of syntax error trailing comma"*.
Top results were dominated by three real, reachable products, inspected below.

## Competitors inspected

### 1. JSONLint — "JSON Error Analyzer" (jsonlint.com/json-error-analyzer)

- **Input:** a single textarea for pasted JSON.
- **Presets:** one-click sample buttons for six broken-JSON shapes — trailing comma, single
  quotes, unquoted key, truncated document, missing comma, undefined value.
- **Output:** line + column (derived from the parser's character offset), visual highlight of
  the offending character, a plain-English explanation of the cause, and a concrete suggested
  fix. Offers an auto-fix for common structural mistakes.
- **Error taxonomy it names:** trailing comma, single-quoted string, unquoted property name,
  missing comma, invalid value (`undefined` / `NaN` / `Infinity`), unescaped control character.
- **Stated limit (good, honest UX):** the reported position is where the parser *detected* the
  problem, not always where the mistake was made — it tells users to check the line above for
  missing-comma cases.

### 2. Jsonic — JSON validator + trailing-comma guide (jsonic.io)

- **Input:** pasted JSON into the validator/formatter panel.
- **Output:** "exact line and column of every syntax error"; the guide emphasises pinpointing the
  exact comma position so the user does not scan the file by hand.
- **Coverage shown:** trailing commas in objects, in arrays, and at any nesting depth.
- **UX:** minimal — paste box plus an action button. No caret/context rendering documented.

### 3. JSON Viewer Tool — JSON Validator (jsonviewertool.com/json-validator)

- **Input:** left editor pane; also file upload and a "Load Sample" button; Tree / Code / Text
  view modes.
- **Output:** line and column of the parse failure with the native-parser message class
  ("unexpected token", "unexpected end of JSON", "invalid escape sequence", "unexpected number").
  Markets itself as "validation plus lint-style feedback" rather than pass/fail.
- **UX controls:** Validate, Format, Minify, Copy result, Download result, Fullscreen, Clear.
- **Limits:** runs client-side ("data stays private"); no hard size cap stated, only a note that
  very large files get slow.

## Table stakes → our decision

| Table stake (seen at ≥1 competitor) | Decision | Where it lands |
| --- | --- | --- |
| Line + column of the failure | **in-model** | every issue carries `line`, `column`, `offset` |
| Byte/char offset | **in-model** | `offset` field, shown in the report header line |
| Plain-English cause, not just parser jargon | **in-model** | `cause` + `explain` per issue |
| Concrete suggested fix | **in-model** | `fix` per issue |
| Context snippet with a caret under the column | **in-model** | `context_lines` param (0–10, default 2) |
| Named error taxonomy (trailing comma, single quotes, unquoted key, missing comma, invalid literal, control char, bad escape, unterminated string/bracket) | **in-model** | own tolerant scanner classifies all of these |
| Report *every* issue, not only the first | **in-model** | `scan_all` (default true); parsers stop at #1, our scanner keeps going |
| Sample/preset broken-JSON buttons | **in-model** | five `[[example]]` preset chips on the page |
| Machine-readable output for scripts/CI | **in-model** | `format = "json"` alongside `report` |
| "Valid JSON" confirmation for good input | **in-model** | valid path reports value type + counts |
| Honest note that the reported spot ≠ the mistake | **in-model** | stated in the report, the page copy, and an FAQ |
| Copy result / Download result / Reset buttons | **in-model (free)** | generator gives every text page Copy + Reset + Download |
| Client-side / private processing | **in-model (free)** | the whole block is WASM, no network |
| Auto-fix the broken JSON | **out-of-model here** | `blocks/json-repair` already does exactly this; this tool diagnoses and points at it rather than duplicating it |
| Pretty-print / minify buttons in the same widget | **out-of-model here** | `blocks/json-beautify` owns formatting |
| File upload of a `.json` file | **out-of-model** | pure text-field tools take pasted text; the CLI covers files via shell redirection |
| Tree / Code / Text editor view modes, fullscreen, syntax-highlighted editor | **out-of-model** | the generator renders declarative controls; a bespoke code editor is a platform feature, not a per-tool one |
| Inline red squiggle in a live editor | **out-of-model** | same reason — needs an editor component |
| JSON Schema validation (beyond syntax) | **out-of-model here** | `blocks/json-schema-batch-validate` covers schema conformance |

## Duplicate check (done before implementing)

- `blocks/json-beautify` — pretty-print/minify; its only diagnostic is serde_json's raw message
  passed through as `invalid JSON: …`. Not an error locator.
- `blocks/json-repair` — *fixes* malformed JSON; it returns repaired text, not a diagnosis, and
  emits no line/column report.
- `blocks/json-schema-batch-validate`, `blocks/structured-data-validator`,
  `blocks/data-validator`, `blocks/format-validator` — schema/field/format validation, not JSON
  syntax diagnostics.

Conclusion: not a duplicate. This tool's product is the *diagnosis* (position, cause, fix,
caret context, full issue list), which no existing block produces.

## Feasibility spike notes

- serde_json's `Error` exposes `line()`, `column()`, and `classify()`, giving a trustworthy
  first-failure position for free — used as the authoritative parse verdict.
- Reporting *all* issues needs more than a parser (parsers abort at #1), so the core carries its
  own tolerant scanner that walks strings/escapes/structure and records every deviation with its
  own line/column. Pure Rust, no new dependencies, wasm-safe.
- Column counting is by UTF-8 characters (not bytes), matching what editors show.
