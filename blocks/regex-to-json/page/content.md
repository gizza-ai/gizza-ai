## About this tool

**Regex to JSON** turns unstructured lines — log files, command output, exports
— into structured JSON. Write a regular expression with **named capture
groups** and each line becomes one JSON object whose keys are the group names,
in the order they appear in the pattern.

- **Named groups are the schema**: `(?<name>…)` (or Python-style `(?P<name>…)`)
  defines the JSON key; numbered groups are ignored. A pattern with no named
  groups is rejected with a hint.
- **Unmatched lines** are yours to decide: **Skip them** (default), **Keep**
  them as `{"_raw": "<line>"}` so no data is lost, or **Fail** with an error
  naming the first non-matching line — handy for validating a format.
- **All matches per line** emits one object per match instead of one per line —
  ideal for `key=value` pairs repeated on a single line.
- **Coerce types** converts captures that look like plain numbers (`42`,
  `-3.14`), `true`/`false`, or `null` into real JSON types. Values with leading
  zeros (`007`) or scientific notation stay strings so IDs are never mangled.
- **Output**: a pretty JSON array, a compact single-line array, or **NDJSON**
  (one JSON object per line — JSON Lines) for piping into other tools.

### Worked example

Input lines:

```
10.0.0.5 GET /index.html 200 512
10.0.0.9 POST /api/login 401 87
```

Pattern (with **Coerce types** on):

```
(?<ip>\S+) (?<method>[A-Z]+) (?<path>\S+) (?<status>\d{3}) (?<bytes>\d+)
```

Output:

```json
[
  { "ip": "10.0.0.5", "method": "GET", "path": "/index.html", "status": 200, "bytes": 512 },
  { "ip": "10.0.0.9", "method": "POST", "path": "/api/login", "status": 401, "bytes": 87 }
]
```

### Limits and edge cases

- Input is capped at **1 MB** (1,000,000 bytes); larger text is rejected with
  an error.
- The syntax is the [Rust `regex`](https://docs.rs/regex/) flavour — linear-time
  matching with no catastrophic backtracking, but **no look-around**
  (`(?=…)`, `(?<=…)`) and **no backreferences** (`\1`).
- Blank and whitespace-only lines are ignored entirely — they are neither
  parsed nor counted as unmatched. Windows (CRLF) line endings are handled.
- A named group that did not participate in a match (an optional group) is
  emitted as `null`, so every record keeps the same keys.
- If nothing matches, the result is an empty array `[]` (empty output for
  NDJSON) — not an error, unless **Unmatched lines** is set to *Fail*.

Everything runs **locally in your browser** via WebAssembly — neither your text
nor your pattern is uploaded.

## FAQ

<details>
<summary>Why do I get "the pattern has no named capture groups"?</summary>

This tool builds JSON keys from group *names*, so at least one group must be
named: write `(?<status>\d{3})` instead of `(\d{3})`. Both the `(?<name>…)`
and Python's `(?P<name>…)` spellings work. Plain numbered groups are ignored —
if you only want a flat list of matches, the separate regex-extract tool does
that.

</details>

<details>
<summary>What happens to lines that don't match the pattern?</summary>

By default they are skipped. Choose **Keep as `{"_raw": line}`** to carry them
through the output unparsed (so you can spot format drift downstream), or
**Fail with an error** to stop at the first non-matching line — the error names
the 1-based line number and shows the line, which makes it a quick format
validator. Blank lines never count as unmatched.

</details>

<details>
<summary>How does type coercion decide what becomes a number?</summary>

With **Coerce types** on, a capture becomes a JSON number only if it is a plain
integer (`42`, `-7`) or plain decimal (`3.14`); exact `true`/`false` become
booleans and exact `null` becomes null. Deliberately conservative: leading-zero
values (`007`), scientific notation (`1e5`), thousands separators (`1,000`),
and integers too big for 64 bits all stay strings, so identifiers survive
untouched. Off by default — everything is a string.

</details>

<details>
<summary>Can one line produce more than one JSON object?</summary>

Yes — tick **All matches per line**. Instead of taking only the first match,
every match on a line emits its own object. With the pattern
`(?<key>\w+)=(?<value>\S+)` the line `host=web1 region=eu` yields two objects.
The order is left-to-right within each line, top-to-bottom across lines.

</details>

<details>
<summary>Why doesn't my lookbehind or backreference work?</summary>

The engine is the Rust `regex` flavour, which guarantees linear-time matching —
a hostile pattern can never hang the page. The trade-off is that look-around
and backreferences are unsupported and rejected with a syntax error. In
practice a named group around the part you want, plus anchors (`^`, `$`, which
match per line here since each line is matched on its own), covers most
patterns.

</details>
