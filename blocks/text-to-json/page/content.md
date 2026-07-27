## About this tool

Paste a block of loosely-structured plain text and get clean JSON back. One box
handles five shapes and auto-detects which one you pasted:

- **INI / .conf** — `[section]` headers become nested objects, flat keys stay at
  the top level.
- **key=value / .env / .properties** — one pair per line into a flat object;
  `export ` prefixes, `key: value`, and quoted values are all tolerated.
- **logfmt** — space-separated `level=info msg="server started" port=8080` pairs
  become one JSON object per line, with quoted values and bare-key boolean flags.
- **CSV / TSV** — a header row plus data rows become an array of objects; the
  delimiter (`,` `;` tab `|`) is auto-sniffed and RFC-4180 `"a, b"` quoting is
  honored.
- **/etc/passwd-style** — colon-delimited lines map to named fields
  (`name`, `password`, `uid`, `gid`, `gecos`, `home`, `shell`, or a 4-field group).

Lines starting with `#` or `;` are treated as comments and skipped. Everything
runs locally in your browser — nothing is uploaded.

### Options

- **Input format** — leave on *Auto-detect* and it picks the shape for you, or pin
  one (handy when a colon file isn't really passwd, or a single-column CSV needs
  forcing).
- **Detect booleans & numbers** — on by default: unquoted `true`/`false` become
  booleans, integers and decimals become numbers, and an empty value becomes
  `null`. Turn it off to keep every value a lossless string (e.g. to preserve
  leading zeros like `007`). Quoted values always stay strings either way.
- **Pretty-print** — on by default for 2-space indented JSON; turn it off to
  minify to one line.
- **Output as** — *JSON* (the parsed value), *NDJSON* (one compact record per
  line, JSON-Lines), or *Report* (wraps the data in
  `{ detected_format, record_count, data }` so you can see what auto-detect chose).

## Worked examples

**logfmt log lines → array of objects**

```
level=info msg="server started" port=8080
level=error msg="connection refused" code=502
```

becomes

```json
[
  { "level": "info", "msg": "server started", "port": 8080 },
  { "level": "error", "msg": "connection refused", "code": 502 }
]
```

**INI config → nested JSON**

```
app = demo

[server]
host = localhost
port = 8080
```

becomes `{ "app": "demo", "server": { "host": "localhost", "port": 8080 } }`.

**CSV → array of objects (with type inference)**

`name,age,admin` / `Alice,30,true` becomes
`[{ "name": "Alice", "age": 30, "admin": true }]` — the header row supplies the
keys, and `30`/`true` are inferred as a number and a boolean.

## Limits & edge cases

- **Auto-detect is a heuristic.** Ambiguous text (say, one colon-delimited line)
  can be mis-tagged — switch **Output as** to *Report* to see what was chosen, and
  pin **Input format** to override it.
- **CSV needs a header + at least one data row.** A single line errors; pin a
  different format if the data really is one row.
- **Duplicate keys keep the last value.** JSON objects can't repeat a key, so a
  repeated INI/key=value key is overwritten by the later one.
- **Type inference is whole-document, not per-field.** Quote a value (or turn
  *Detect booleans & numbers* off) to force a specific value to stay a string.
- **Everything is parsed in memory** in your browser tab, so very large pastes can
  be slow.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>How does auto-detect decide which format I pasted?</summary>

It scans the non-comment lines in order: a `[section]` header means INI; a line
packing two or more space-separated `key=value` pairs means logfmt; consistent
colon-delimited lines with no `=` mean passwd; lines that mostly carry one `=` or
`:` per line mean key=value; and a delimiter that splits rows into a consistent
2-plus columns means CSV. If nothing matches it falls back to key=value. Set
**Output as** to *Report* to see the `detected_format` it landed on, and pin
**Input format** if the guess is wrong.

</details>

<details>
<summary>Why did my numbers or booleans come out as strings (or vice versa)?</summary>

By default **Detect booleans & numbers** is on, so unquoted `true`/`false` become
booleans and numeric values become numbers. Two things keep a value a string: it
was **quoted** in the source (`port = "8080"` stays `"8080"`), or you turned the
option **off** to preserve values exactly — useful for zip codes or IDs with
leading zeros like `007`. An empty value becomes `null` when detection is on.

</details>

<details>
<summary>What's the difference between JSON, NDJSON, and Report output?</summary>

**JSON** is the parsed value itself — a nested object for INI and key=value, an
array of objects for logfmt, CSV, and passwd. **NDJSON** (JSON-Lines) emits one
compact record per line, which is what many log and data pipelines ingest.
**Report** wraps the result as `{ detected_format, record_count, data }` so you
can confirm which format auto-detect chose and how many records it found.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly — the text you
paste never leaves your machine, so it's safe for config files, credentials, or
log lines you'd rather not send to a server.

</details>

<details>
<summary>Can it handle CSV with commas inside quoted fields, or tab/semicolon files?</summary>

Yes. The delimiter is auto-sniffed from comma, semicolon, tab, and pipe, and
RFC-4180 double-quoting is honored — `"a, b"` stays a single field and `""`
inside a quoted field becomes a literal `"`. Blank column headers fall back to
`field1`, `field2`, and so on. Pin **Input format** to *CSV / TSV* if a table
with only one column would otherwise be read as key=value.

</details>
