## About this tool

JSON Lines and NDJSON files are common in logs, event streams, warehouse exports, and model datasets. This profiler gives you a quick schema-and-quality snapshot without loading the file into a database: record count, key presence frequency, coverage percentage, JSON value types, distinct scalar counts, sample values, numeric ranges, and string length ranges.

Set depth to `1` for top-level keys or increase it to inspect dotted nested paths such as `user.id` and array element paths such as `items[].sku`. Choose text for a quick read, JSON for automation, Markdown for documentation, or CSV for spreadsheets.

### Worked example

Input:

```jsonl
{"id":1,"status":"ok","latency_ms":12}
{"id":2,"status":"error","latency_ms":940,"err":{"code":"timeout"}}
{"id":3,"status":"ok","latency_ms":31}
{"id":4,"status":"ok"}
```

Default text output starts with:

```text
records: 4 · lines read: 4 · invalid: 0
record types: object 4
keys: 4 (depth 1)
```

The `latency_ms` row shows `present 3`, `coverage 75%`, `number 3`, and numeric min/max/mean values.

## Limits and edge cases

- Up to 50,000 non-blank lines are accepted per run.
- Each line must be one complete JSON value. Blank lines are ignored.
- Top-level non-object records count toward record-type totals, but only object records contribute key statistics.
- Distinct scalar tracking is capped internally; after the cap the report shows `10000+`.
- Nested depth is capped at 10. Array elements use `[]` in the path so repeated objects count as one record carrying that path.
- `invalid=report` records parse-error examples, `skip` only counts them, and `error` stops at the first bad line.

## FAQ

<details>
<summary>What is the difference between JSONL and NDJSON?</summary>

They are the same practical shape for this tool: one JSON value per line. The line-by-line layout lets logs and data pipelines append records without wrapping the whole file in a JSON array.

</details>

<details>
<summary>Does key coverage count array occurrences?</summary>

No. Coverage counts records. If one record has an `items` array with ten objects that all contain `sku`, the nested path `items[].sku` is counted as present in one record, not ten occurrences.

</details>

<details>
<summary>Can this infer a formal JSON Schema?</summary>

No. It reports observed key coverage, types, distinct counts, samples, and simple value ranges. That is useful input for schema design, but it does not generate required/optional JSON Schema constraints automatically.

</details>

<details>
<summary>Why are top-level arrays or strings counted but not listed as keys?</summary>

The record-type summary includes every valid JSON value. Per-key statistics only apply to object records because arrays and scalars do not have named fields at the top level.

</details>
