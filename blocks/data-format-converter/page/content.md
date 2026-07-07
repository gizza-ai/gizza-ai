## CSV · TSV · JSON · NDJSON converter

Paste your data, pick a target format, and convert it — CSV, TSV, JSON, and
NDJSON (JSON Lines) in any of the twelve directions. Nothing is uploaded: the
conversion is compiled to WebAssembly and runs locally in your browser tab, so
your data never leaves your device.

### How it works

- **From** — leave it on **Auto-detect** and the tool sniffs the input: text
  starting with `[` is a JSON array, `{` is a single JSON object (or NDJSON when
  every line is its own JSON value), and anything else is parsed as CSV — or TSV
  when the first row is tab-separated. Force a source format if you need to.
- **To** — choose **JSON** (a single array), **NDJSON / JSONL** (one compact
  record per line, ideal for streaming, log pipelines, MongoDB `mongoimport`,
  BigQuery, and JSONL training sets), **CSV**, or **TSV**.
- **First row is headers** — for CSV/TSV, treat the first row as field names. On
  (the default) you get an array of objects keyed by the headers; off gives an
  array of arrays. When writing CSV/TSV it emits a header row from the union of
  every record's keys, in first-seen order, filling blanks where a row is missing
  a key.
- **Infer types** — CSV/TSV cells become JSON numbers, `true`/`false`, and `null`
  for empty cells. Turn it off to keep every cell as a string. Values that would
  not round-trip — leading zeros like `007`, a leading `+` — always stay strings,
  so zip codes and phone numbers survive intact.
- **Pretty-print** — indent the **JSON** target. NDJSON is always one compact
  record per line, so this option does not affect it.
- **Flatten** — when writing CSV/TSV, expand nested objects and arrays into
  dot-notation columns (`{"addr":{"city":"NYC"}}` becomes an `addr.city` column)
  instead of writing them as compact JSON strings.

Quoted fields, embedded commas/tabs, and newlines are handled per RFC 4180.

### Worked example

Input CSV:

```
name,age,active
Alice,30,true
Bob,25,false
```

To **NDJSON** →

```
{"name":"Alice","age":30,"active":true}
{"name":"Bob","age":25,"active":false}
```

To **JSON** → `[{"name":"Alice","age":30,"active":true},{"name":"Bob","age":25,"active":false}]`

Going the other way, `{"a":1,"b":2}` / `{"a":3,"c":4}` as **NDJSON → CSV** gives
`a,b,c` with a blank cell where the second row has no `b`.

### Limits & scope

- Everything runs in your browser, so the practical size limit is your device's
  memory — there is no fixed cap, but multi-hundred-MB files may be slow.
- This tool covers the four named formats. For semicolon- or pipe-delimited files,
  or arbitrary delimiter swaps, use a dedicated delimiter converter.
- Flattening applies to CSV/TSV **output**; it does not rebuild nested objects
  from dot-notation headers on input.

### FAQ

<details>
<summary>What is NDJSON / JSONL, and how is it different from JSON?</summary>

NDJSON (newline-delimited JSON), also called JSON Lines or JSONL, puts one
complete JSON value on each line with no surrounding array. It streams
record-by-record, so tools can process huge datasets without loading the whole
file — which is why `mongoimport`, BigQuery, Kafka, and LLM training pipelines
prefer it. Standard JSON here is a single array holding every record.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely inside your
browser tab. Nothing is sent to a server.

</details>

<details>
<summary>Why did a number stay a string?</summary>

Values with a leading zero (like `007`) or a leading `+` are kept as text so
they survive the round trip unchanged — that preserves zip codes, phone numbers,
and product codes. You can also turn **Infer types** off to keep every cell as a
string.

</details>

<details>
<summary>Does it support TSV, and how do I convert between CSV and TSV?</summary>

Yes — TSV is a first-class format. Pick **TSV** as the source or target (or let
auto-detect handle a tab-separated first row). Setting From = CSV and To = TSV
(or the reverse) converts between the two while fixing up quoting for the new
delimiter.

</details>

<details>
<summary>My records have different keys — what happens in the table output?</summary>

When writing CSV/TSV from JSON or NDJSON, the header row is the union of every
record's keys in first-seen order. Rows missing a key get a blank cell, so no
data is dropped and columns line up across records.

</details>
