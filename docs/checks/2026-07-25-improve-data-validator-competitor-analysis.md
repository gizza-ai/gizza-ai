# data-validator — competitor analysis (2026-07-25)

Function: validate pasted CSV or JSON rows against field rules (required, unique,
type, range, length, regex, enum) and report violations locally.

## Competitors scanned

1. **CSV Lint / CSVLint-style validators** — table-focused CSV validation. Common
   strengths: header checks, type checks, required columns, clear row/column
   errors, and delimiter handling. Some variants depend on external schemas.
2. **JSON Schema validators** — validate JSON data against a schema with required
   fields, types, enums, regex/patterns and min/max constraints. Strong for JSON,
   but not convenient for CSV rows without a conversion step.
3. **OpenRefine / data quality tools** — profiling and cleaning workflows with
   facets, duplicate checks, type coercion and filters. Powerful but heavier than
   a one-shot browser-local validator.
4. **CSV validators in spreadsheet/import products** — preview import failures,
   list row numbers, bad values and rule messages. Usually tied to a specific app
   or upload flow.

## Table-stakes → decision

| Capability | Fit | Decision |
|---|---|---|
| CSV input with delimiter handling | in-model | `data` + `delimiter=auto|comma|tab|semicolon|pipe` |
| JSON array/object/NDJSON input | in-model | `input_format=json` or auto-detect |
| Required fields | in-model | `field:required` |
| Type checks | in-model | `type=int|float|bool|date|email|url` plus shorthand |
| Numeric min/max | in-model | `min=` / `max=` |
| String length min/max | in-model | `minlen=` / `maxlen=` |
| Regex / pattern checks | in-model | `regex=` with user-supplied anchors |
| Enum / allowed values | in-model | `enum=a|b|c` |
| Unique values | in-model | `unique` reports later duplicates |
| Row/line/field/value in error list | in-model | violation report fields |
| JSON output for automation | in-model | `format=json` |
| Rule presets / examples | in-model | page example chips |
| Full JSON Schema support | out-of-model | listed; use a dedicated JSON Schema validator |
| Cross-field formulas / SQL predicates | out-of-model | listed; beyond simple field rules |
| Auto-repair / transformation | out-of-model | listed; this is report-only |
| Remote datasets / database validation | out-of-model | listed; pasted local data only |

## Out-of-model (considered, not built)

- Complete JSON Schema or CSVW schema support: valuable but a separate schema
  language, not this simple one-rule-per-line model.
- Cross-field formulas, conditional required rules, joins or lookups: would need a
  richer expression engine and more inputs.
- Auto-fixing data: intentionally not built; the tool reports violations without
  modifying user data.
