# json-coerce-types — competitor analysis (2026-08-07)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is a
**paraphrase** of observed behaviour and option names; no competitor copy, branding or
trademarks were reused. Out-of-model items are recorded, not built.

## Scope of the tool

Take a **valid but loosely-typed** JSON document — the kind produced by CSV importers, form
posts, `application/x-www-form-urlencoded` bodies, spreadsheet exports and older APIs, where
every scalar arrives quoted — and coerce `"42"` → `42`, `"true"` → `true`, `"null"` → `null`
recursively, leaving everything that isn't safely coercible alone.

Deliberately **not** the same job as the neighbouring blocks (dup check, step 2):

| Existing block | What it does | Why this is different |
|---|---|---|
| `json-repair` | Fixes **malformed** JSON (trailing commas, comments, unquoted keys). Its bareword path turns `{a: 42}` into a number, but a *quoted* `"42"` stays a string — verified in `core/src/lib.rs` `word_value`/`number_value`. | We start from *valid* JSON and retype quoted scalars. |
| `json-beautify` | Validates + pretty/minifies. Types untouched. | No coercion at all. |
| `json-remove-nulls` | Deletes null/empty keys. | We *create* nulls, never delete keys. |
| `csv-type-inferrer` | Infers column types **from CSV text**. | Different input format; per-column inference, not per-value coercion of an existing JSON tree. |
| `json-transform-rules` | Declarative `target = selector` remapping; has a `number`/`boolean` transform, but only per explicitly named path. | We do a whole-tree sweep with no rules to author. |

Conclusion: **not a duplicate** — build it.

## Competitors reviewed (top 3 real)

### 1. `auto-parse` (npm, greenpioneersolutions) — the closest functional match
- Recursive traversal of arrays and plain objects; returns new copies, never mutates.
- Coerces numeric strings and boolean strings **case-insensitively** (`'TrUe'` → `true`).
- Opt-in boolean synonyms: `yes` / `no` / `on` / `off`.
- `preserveLeadingZeros` — keeps `'0005'` as a string instead of `5`.
- `parseCommaNumbers` — `'385,134'` → `385134`.
- `allowedTypes` — restrict which target types coercion may produce.
- `stripStartChars` — strip a prefix such as `#` before parsing.
- BigInt support; circular references detected and returned unchanged.
- **Notable default hazard:** leading zeros are coerced by default, which silently destroys
  zip codes, phone numbers and zero-padded IDs.

### 2. Hosted CSV→JSON converters with an "Infer data types" toggle (LimeConvert and peers)
- One master **type-inference checkbox**: numeric strings → numbers, `true`/`false` →
  booleans, empty values → `null`; off keeps everything as text.
- Indentation as a number field where **0 = minified**.
- Output-shape presets exposed as one-click buttons (array of objects, array of arrays,
  column-keyed object, NDJSON).
- Delimiter auto-detection with manual override.
- Copy + download buttons; "runs in your browser, data never leaves the device" positioning;
  no stated size limit.

### 3. `jq` `walk` + `tonumber?` / `toboolean?` (the CLI-native idiom)
- `tonumber` / `toboolean` parse a correctly-formatted string, pass through an already-typed
  value, and **error** on anything else.
- The cookbook idiom is a try/fallback wrapper — `def tonumberq: tonumber? // .;` — composed
  with `walk` for whole-tree coercion.
- jq is strict: strings and numbers are never implicitly coerced, so every conversion is
  explicit and opt-in per type.
- The published recipe also trims string values and maps empty strings to `null`.
- Cost: it is a program you have to write correctly, with no per-key escape hatch.

## Table stakes → where each one landed

| Table stake | Source | Decision |
|---|---|---|
| Recursive coercion through nested objects **and** arrays | all 3 | **in-model** — whole-tree walk |
| Numeric strings → numbers | all 3 | **in-model** — `numbers` (default on) |
| `"true"`/`"false"` → booleans, case-insensitive | 1, 2 | **in-model** — `booleans` (default on) |
| `"null"` → `null` | 2, 3 | **in-model** — `nulls` (default on) |
| Boolean synonyms (`yes`/`no`/`on`/`off`) | 1 | **in-model** — `bool_synonyms` (default off) |
| Extra null tokens (`NA`, `N/A`, `-`) | 3 (recipe) | **in-model** — `null_tokens`, matching `csv-type-inferrer`'s param of the same name (family consistency) |
| Empty string → `null` | 2, 3 | **in-model** — `empty_strings` enum `keep`\|`null` |
| Trim whitespace before testing | 3 | **in-model** — `trim` (default off) |
| Preserve leading zeros | 1 | **in-model** — `leading_zeros` enum, **defaulting to `keep`** (we deliberately invert competitor 1's hazardous default; `coerce` is opt-in) |
| Thousands separators (`"1,234.5"`) | 1 | **in-model** — `thousands` (default off) |
| Per-type opt-out | 1 (`allowedTypes`), 3 | **in-model** — the three type checkboxes are the opt-out |
| Pretty/minify output, 0 = minify | 2 | **in-model** — `indent` 0–8 |
| Preset one-click examples | 2 | **in-model** — three `[[example]]` chips |
| Copy / download / local-only | 2 | platform — the generator already ships Reset, Copy and a Download link on `format = "text"` pages |
| Per-key escape hatch (never coerce `zip`, `phone`, `id`) | gap in **all 3** | **in-model, our differentiator** — `skip_keys` / `only_keys` |
| Change report | gap in all 3 | **in-model, our differentiator** — `output = report` lists every coerced path with before → after |
| Big-integer / precision safety | 1 (BigInt) | **in-model, our differentiator** — an integer string too large for `i64`/`u64` is **left as a string** rather than silently rounded through `f64` |

## Considered, not built (out-of-model or rejected)

- **BigInt output type** (competitor 1) — JSON has no BigInt; emitting one is a JS-runtime
  concept. Our precision-safe fallback (leave the string alone) is the honest JSON answer.
- **Circular-reference detection** (competitor 1) — impossible in JSON input by construction.
- **`stripStartChars`** (competitor 1) — a string-mangling step, not type coercion; already
  covered by `find-replace` upstream. Rejected to avoid schema bloat.
- **CSV/delimiter input and output-shape presets** (competitor 2) — that is exactly
  `csv-json-convert` + `csv-type-inferrer`; adding a CSV front-end here would duplicate two
  shipped blocks.
- **File drag-and-drop upload** (competitor 2) — the pure-tool page surface is a textarea;
  input is capped at 5 MB and the limit is stated on the page.
- **Date/timestamp detection** — JSON has no date type, so "coercion" would mean rewriting
  strings into other strings. Rejected; stated as a limit on the page instead.
- **Arbitrary jq programs** (competitor 3) — a full expression language is `jsonata-query` /
  `jsonpath-query` / `json-transform-rules` territory.

## Sources

- [auto-parse — npm](https://www.npmjs.com/package/auto-parse) ·
  [GitHub](https://github.com/greenpioneersolutions/auto-parse)
- [parse-strings-in-object — npm](https://www.npmjs.com/package/parse-strings-in-object)
- [LimeConvert CSV to JSON](https://limeconvert.com/csv-to-json)
- [jq 1.8 manual](https://jqlang.org/manual/) · [jq cookbook](https://github.com/jqlang/jq/wiki/Cookbook)
- [Online JSON Tools](https://onlinetools.com/json)
