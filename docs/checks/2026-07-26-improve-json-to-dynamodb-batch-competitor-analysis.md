# Competitor analysis — json-to-dynamodb-batch (2026-07-26)

Function: convert a JSON object / array of items into a DynamoDB `BatchWriteItem`
request payload with typed attribute values (marshalling). Research done before
implementing; all findings paraphrased — no competitor copy/branding reproduced.

## Competitors scanned (top real tools)

### 1. Dynobase — JSON ⇄ DynamoDB JSON converter
- Two-way: plain JSON → marshalled DynamoDB JSON, and back.
- Full attribute-type coverage: S, N, BOOL, NULL, M, L, SS, NS, B.
- Numbers marshalled as strings (`3` → `{"N":"3"}`); nesting supported.
- Free, browser-based; part of a wider DynamoDB tool suite (CSV/JSON import,
  export to CSV/JSON/S3 — those are server/account features, out of model here).

### 2. Blowstack — DynamoDB two-way converter
- Bidirectional (JSON/JS ⇄ DynamoDB JSON) with a side-by-side live panel.
- Full type coverage incl. Binary Set (BS): S, N, BOOL, NULL, B, L, M, SS, NS, BS.
- Handles deeply nested rows.
- Extras: built-in **item size calculator**, a data-type **legend/reference**,
  and docs showing how to feed the output to the AWS CLI `batch-write-item`.
- Free.

### 3. ddbjson (CLI, duartealexf)
- Terminal marshall/unmarshall between plain and DynamoDB JSON.
- Input via file, string arg, or stdin.
- `-g` dot-path filter to convert only a subset (object props, array indices,
  wildcards).
- Aimed at piping into AWS CLI (`get-item`, `scan`); no dedicated
  batch-write-item wrapper or table-name handling.

## Table-stakes → decisions (in-model vs out-of-model)

| Capability | Competitors | Our decision |
| --- | --- | --- |
| Full type marshalling S/N/BOOL/NULL/M/L | all | **in** — recursive marshaller |
| Numbers as strings | all | **in** — `n.to_string()`, precision preserved |
| String/Number Sets (SS/NS) | dynobase, blowstack | **in** — `sets=auto` opt-in (JSON has no Set type, so off by default → arrays are Lists) |
| Binary (B) / Binary Set (BS) | dynobase, blowstack | **out** — JSON has no binary marker; can't auto-detect a base64 string vs a plain string safely. Noted on page. |
| `BatchWriteItem` `RequestItems` payload | (docs only) | **in** — this is our core differentiator; competitors emit bare marshalled JSON, not the request envelope |
| Table name | ddbjson: no | **in** — `table` param keys `RequestItems` |
| 25-item batch limit / chunking | none | **in** — `chunk` splits >25 items into multiple payloads (real AWS hard limit) |
| Put vs Delete requests | none | **in** — `request_type` + `key_attributes` for DeleteRequest keys |
| Empty-string handling | (implicit) | **in** — `empty_strings` keep/null/skip (DynamoDB now allows empty strings) |
| Pretty vs compact output | typical | **in** — `pretty` toggle |
| Bare-marshall output (like the converters) | dynobase, blowstack | **in** — `output=items` / `request-list` modes |
| Reverse (unmarshall DynamoDB→JSON) | dynobase, blowstack | **out** for this tool — this is a one-way generator; reverse is a separate tool candidate |
| Item size calculator | blowstack | **considered, rejected** — nice-to-have, not core to producing a payload; adds UI surface |
| dot-path subset filter | ddbjson | **considered, rejected** — a jsonpath/jq tool already exists in the toolkit; keep this tool focused |

Out-of-model (need server/account/keys): live import into a real table, S3/CSV
export, AWS credentials — none fit browser-local wasm.
