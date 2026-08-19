## About this tool

NDJSON (also called JSON Lines) is the format many loggers, queues and export jobs use when they
want one JSON record per line. It is friendly to streaming tools, but not friendly to eyeballs: one
broken line can hide inside thousands of records, nested fields are hard to scan, and turning the
stream into an array or a table usually means writing a one-off script.

This viewer keeps the streaming shape but makes it readable. Paste a stream and choose one of four
views:

| View | Use it when |
|---|---|
| Pretty | You want one indented record per block, with key order preserved. |
| Compact | You want valid NDJSON back, minified one record per line. |
| Array | A downstream tool accepts JSON arrays but not JSON Lines. |
| Table | Records are mostly objects and you want a quick column view. |

Search walks every key and scalar value at any depth, so `timeout` finds `err.code` without you
knowing the path. When you do know the path, use a dotted selector like `user.name` or
`items.0.id`. Invalid lines are reported in place by default, with line and column, so you can fix
the bad record without losing the rest of the stream.

### Worked example

Paste this stream:

```json
{"id":1,"status":"ok","latency_ms":12}
{"id":2,"status":"error","latency_ms":940,"err":{"code":"timeout"}}
{"id":3,"status":"ok","latency_ms":31}
```

Set **Search any key or value** to `timeout`, turn on **Show input line numbers**, and leave the
view on **Pretty**. The result is only the matching record, with the original line number preserved:

```json
# record 1 (line 2)
{
  "id": 2,
  "status": "error",
  "latency_ms": 940,
  "err": {
    "code": "timeout"
  }
}
```

### Limits and edge cases

- Up to **50,000 non-blank lines** per run.
- Every non-blank line must be one complete JSON value. Pretty-printed multi-line JSON is JSON, not
  NDJSON; use the array view after first converting it to one record per line.
- Table view is a quick text table, not CSV. Nested objects and arrays are rendered as compact JSON
  inside a cell.
- Search is text-based over keys and scalar values. For a boolean expression language or field
  projection, use a dedicated NDJSON filter tool instead.

## FAQ

<details>
<summary>What is the difference between NDJSON and a JSON array?</summary>

NDJSON has one complete JSON value per line, with no comma between lines and no outer brackets. It
is easy to append to and stream because a program can parse each line as soon as it arrives. A JSON
array wraps all records in `[` and `]` with commas between them, which is what many web APIs and
editors expect. The **Array** view turns the lines into that shape without changing the records.

</details>

<details>
<summary>Will one bad line make the whole stream fail?</summary>

Not by default. **Lines that are not valid JSON** is set to **Report each one in place**, so the
viewer prints a `# line N: invalid JSON` marker and keeps rendering the other records. Switch it to
**Stop at the first one** when you are validating a machine pipeline and want a non-zero failure
instead, or **Drop them silently** when you are exploring a noisy capture.

</details>

<details>
<summary>How do dotted paths work?</summary>

A path segment walks an object key, and a numeric segment indexes an array. `user.name` reads the
`name` key inside the `user` object. `items.0.id` reads the `id` key from the first item in the
`items` array. If the path is present and the value box is empty, the path is an existence filter;
if the value box is filled, that exact field is compared using the selected match mode.

</details>

<details>
<summary>Does table view flatten nested JSON?</summary>

No. The table unions top-level keys across the shown object records. If a cell is a nested object or
array, that nested value is printed as compact JSON inside the cell. That keeps the table faithful
and predictable; deep flattening needs choices about separators, arrays and duplicate keys that are
better handled by a converter built for tabular output.

</details>
