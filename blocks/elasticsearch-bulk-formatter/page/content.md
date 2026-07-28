## About this tool

The Elasticsearch Bulk Formatter builds the exact newline-delimited JSON body used by the
Elasticsearch and OpenSearch `_bulk` APIs. Paste a JSON array of objects, choose the bulk
action, and the tool emits compact NDJSON: one action/metadata line per document and, for
`index`, `create`, and `update`, one source line after it. The output always ends with the
required trailing newline.

### Worked example

Input documents:

```json
[{"id":"1","title":"hello"},{"id":"2","title":"world"}]
```

Options: action `index`, target `_index` `my-index`, ID field `id`.

Output:

```json
{"index":{"_index":"my-index","_id":"1"}}
{"title":"hello"}
{"index":{"_index":"my-index","_id":"2"}}
{"title":"world"}
```

The final line in the actual output is followed by `\n`, so it is safe to save directly as an
NDJSON file and send with `curl --data-binary`.

### Options

- **Documents JSON array** — a JSON array of objects. Each object becomes one operation.
- **Bulk action** — `index`, `create`, `update`, or `delete`.
- **Target `_index`** — written into every metadata line. Leave blank when you plan to POST to
  `/<index>/_bulk` and want the URL to supply the index.
- **Document ID field** — the field whose value becomes `_id`; that field is removed from the
  emitted source document. It is required for `update` and `delete` and optional for `index` and
  `create`.
- **doc_as_upsert** — for `update`, adds `"doc_as_upsert": true` beside `"doc"` so missing
  documents are inserted from the partial document. It is ignored for other actions.

### Limits and edge cases

This tool only shapes the request body. It does not connect to a cluster, validate mappings,
chunk large files into upload-size batches, or add per-document routing/version metadata. It
errors on invalid JSON, a non-array root, an empty array, a non-object item, and an `update` or
`delete` action without an `_id` value. Numeric IDs stay numeric in the metadata line; string IDs
stay strings.

## FAQ

<details>
<summary>Why does the output have a blank-looking line at the end?</summary>

The `_bulk` format requires the body to end with a newline. The tool always appends that final
`\n`; many command-line displays make it look like there is an extra blank line, but it is the
correct delimiter for Elasticsearch and OpenSearch.

</details>

<details>
<summary>Should I include `_index` in the body or in the URL?</summary>

Both patterns are valid. Fill **Target `_index`** to write `_index` on every action line. Leave it
blank when you are posting to a URL such as `/my-index/_bulk` and want the endpoint to supply the
default index.

</details>

<details>
<summary>Why is my ID field removed from the source document?</summary>

When you set **Document ID field**, that field is promoted to `_id` in the action metadata and
removed from the source line. This avoids storing a duplicate application ID field unless you
explicitly want it. Leave the field blank for `index` or `create` if you want Elasticsearch to
generate IDs and keep all document fields.

</details>

<details>
<summary>Can this send the bulk request to my cluster?</summary>

No. It is intentionally browser-local and offline: it produces the NDJSON body only. Save or copy
the result, then send it with your own `curl`, Kibana Dev Tools, client library, or deployment
pipeline.

</details>
