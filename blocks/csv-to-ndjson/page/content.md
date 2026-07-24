## About this tool

**NDJSON** (newline-delimited JSON, also written `.ndjson` or `.jsonl`) puts one complete JSON
value on each line, with no enclosing array. That's the format streaming tools want: you can
process a huge file line-by-line without loading it all into memory, `cat` two files together,
`tail -f` a stream, or feed it straight into `jq -c`, a BigQuery load job, an Elasticsearch bulk
request, or a log shipper.

This converter parses your CSV — including quoted fields with embedded commas, newlines, and
`""` escapes (RFC 4180) — and emits one JSON object per data row. Everything runs locally in
your browser: your data never leaves the page.

### Worked example

Input CSV:

```
name,age,active
Ada,36,true
Grace,,false
```

Output NDJSON (defaults — every value stays a string, so nothing is lost):

```
{"name":"Ada","age":"36","active":"true"}
{"name":"Grace","age":"","active":"false"}
```

Turn on **Parse numbers** and **Parse true/false/null** and the same input becomes typed:

```
{"name":"Ada","age":36,"active":true}
{"name":"Grace","age":null,"active":false}
```

Untick **First row is headers** and each row is emitted as a JSON array instead of an object:
`["Ada","36","true"]`.

### Options

- **Delimiter** — `,` by default; also accepts `;`, `|`, the word `tab`, or any single
  character (great for TSV exports).
- **First row is headers** — on: keys come from the header row (objects). Off: rows become
  arrays.
- **Parse numbers** — numeric cells become JSON numbers. Values with leading zeros or a leading
  `+` (`007`, `+1`) stay strings, so zip codes and ids survive.
- **Parse true/false/null** — the literals `true`, `false`, and `null` (and empty cells) become
  JSON booleans / `null`.
- **Trim whitespace** — strips spaces around each cell before conversion.

## FAQ

<details>
<summary>What's the difference between NDJSON, JSONL, and a normal JSON array?</summary>

They describe the same idea: **NDJSON** and **JSONL** both mean "one JSON value per line, no
wrapping array." A normal JSON array (`[ {...}, {...} ]`) must be parsed as a whole; NDJSON can
be read one line at a time, which is why streaming and big-data ingest tools prefer it. This
tool outputs the line-delimited form; use the CSV to JSON tool if you want a single array.

</details>

<details>
<summary>Why are my numbers and booleans coming out as strings?</summary>

By default every cell is emitted as a JSON string, so nothing is silently reinterpreted. Tick
**Parse numbers** to turn numeric cells into JSON numbers and **Parse true/false/null** to turn
those literals into real booleans/null. Leaving them off is the safe, lossless choice for
pipelines that do their own typing.

</details>

<details>
<summary>Will it handle commas and newlines inside a quoted field?</summary>

Yes. Fields wrapped in double quotes may contain the delimiter, line breaks, and escaped quotes
(`""` → `"`), following RFC 4180. So `"Ada, L"` stays a single value and an embedded newline is
encoded as `\n` in the JSON output.

</details>

<details>
<summary>How do I convert a tab-separated (TSV) file?</summary>

Set the delimiter to the word `tab` (or paste an actual tab character). Any single character
works as a delimiter, so `;` and `|` exports convert the same way.

</details>

<details>
<summary>Is there a size limit and does my data get uploaded?</summary>

Nothing is uploaded — the conversion happens entirely in your browser via WebAssembly, so your
data stays on your machine. Because it all runs in memory, extremely large files (hundreds of MB)
may be slow or hit the browser tab's memory limit; for those, a command-line NDJSON converter is
a better fit.

</details>
