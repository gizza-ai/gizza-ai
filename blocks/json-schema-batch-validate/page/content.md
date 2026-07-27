## About this tool

Paste one **JSON Schema** and a whole **batch of records**, and this tool validates every record
against that schema in one pass. The batch can be a JSON array of records, a single JSON value
(treated as one record), or an **NDJSON** stream (one JSON value per line). You get a `PASS`/`FAIL`
verdict, a summary of how many records passed and failed, and — for each failing record — the exact
errors, each with a **JSON Pointer path**, the schema **keyword** that rejected the value, and a
human-readable message.

It runs entirely in your browser as WebAssembly: nothing is uploaded, and no network `$ref`s are
fetched. Choose the JSON Schema draft label explicitly, or leave it on **Auto** to detect the draft
from the schema's `$schema` keyword (falling back to Draft 2020-12). Validation is intentionally
local and covers the common draft-agnostic keyword subset listed in the limits below.

### Worked example

**Schema**

```json
{"$schema":"http://json-schema.org/draft-07/schema#",
 "type":"object","required":["id","name"],
 "properties":{"id":{"type":"integer"},"name":{"type":"string"}}}
```

**Records** (JSON array)

```json
[{"id":1,"name":"Ada"},{"id":"2","name":"Grace"},{"name":"Kay"}]
```

**Result**

```
Batch validation: FAIL
Draft: draft7 (from $schema)    Input: json    Records: 3 (1 passed, 2 failed)
Total errors: 2

Record #1 — 1 error
  /id: "2" is not of type "integer"

Record #2 — 1 error
  (root): "id" is a required property
```

Record 0 passes. Record 1 fails because `id` is a string, not an integer. Record 2 fails because
the required `id` property is missing. Switch **Output format** to *JSON* for the same result as a
machine-readable object you can pipe into another tool.

## FAQ

<details>
<summary>What batch formats can I paste into the records box?</summary>

Three shapes are accepted. A **JSON array** `[{...},{...}]` validates each element as a record. A
**single JSON value** (object, number, string, …) is validated as one record. **NDJSON** (also
called JSON Lines) is one JSON value per line, with blank lines skipped. Leave *Records format* on
**Auto** to try whole-document JSON first and fall back to NDJSON, or force `json` / `ndjson` when
you know the shape.

</details>

<details>
<summary>Which JSON Schema drafts are supported, and how does auto-detect work?</summary>

Draft 4, Draft 6, Draft 7, Draft 2019-09, and Draft 2020-12 can be selected for reporting. On
**Auto**, the tool reads the schema's `$schema` URI and picks the matching draft label; if there is
no `$schema` (or it isn't recognized) it falls back to Draft 2020-12. The validator applies the
same local keyword subset for each draft, so use a full validator when you need draft-specific
features such as `oneOf`, conditionals, unevaluated properties, or `format` assertions.

</details>

<details>
<summary>What does each reported error contain?</summary>

Every error carries three things: a **JSON Pointer path** to the failing value (`` empty / shown as
`(root)` for the record itself, `/age` for a member, `/tags/0` for the first array item), the
schema **keyword** that rejected it (`type`, `required`, `minimum`, `enum`, `pattern`, …), and a
message. Errors within a record are sorted by path so the output is stable. The per-record and
total error **counts** are always exact, even when the listing is capped.

</details>

<details>
<summary>What is "Max errors to list" for?</summary>

It caps how many individual errors are printed across the whole batch (default 50) so a huge, badly
broken batch stays readable. Only the *listing* is capped — the per-record `error_count` and the
batch `total_errors` are still counted in full, and the report is marked **truncated** when more
errors exist than were shown.

</details>

<details>
<summary>Does it fetch remote schemas or `$ref` URLs?</summary>

No. The tool is fully local: it never makes network requests, so external HTTP `$ref`s are not
resolved. Internal `$ref`s inside the schema you paste (references within the same document) are
resolved normally. If your schema depends on remote definitions, inline them before validating.

</details>

## Limits & edge cases

- **Local only** — no network, so remote/HTTP `$ref`s are not fetched (internal `$ref`s work).
- **Keyword subset** — validates `type`, `required`, `properties`, `additionalProperties`, `enum`,
  `const`, numeric bounds, string length, `pattern`, `items`, and internal `$ref`. Complex
  composition/conditional keywords and `format` assertions are treated as annotations.
- An **empty batch** (`[]`) is reported as valid with 0 records.
- Passing records are counted in the summary but not listed individually; only failing records get
  a detailed error breakdown.
- `format` keyword handling follows the selected draft's rules; when in doubt, pick the draft
  explicitly rather than relying on auto-detect.
- Very large batches are still validated in full, but the error **listing** is bounded by
  *Max errors to list*.
