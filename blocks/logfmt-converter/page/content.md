## logfmt · JSON · NDJSON · CSV converter

Paste a block of log lines, pick a target format, and convert it. Unlike most
logfmt tools, this one converts in **both** directions: it parses logfmt *and*
writes it, so you can take JSON, NDJSON (JSON Lines), or a CSV export and turn it
back into `key=value` log records. Nothing is uploaded — the converter is
compiled to WebAssembly and runs locally in your browser tab.

### How it works

- **From** — leave it on **Auto-detect** and the input is sniffed: text starting
  with `[` is a JSON array, `{` is a single JSON object (or NDJSON when every
  line is its own JSON value), a leading `key=value` token means logfmt, and
  anything else is parsed as CSV. Force a source format if detection guesses
  wrong on an unusual sample.
- **To** — choose **logfmt**, **JSON** (a single array), **NDJSON / JSONL** (one
  compact record per line), or **CSV**.
- **CSV delimiter** — comma, semicolon, tab, or pipe. It applies to CSV on both
  sides, so you can re-delimit a file by setting From and To to CSV.
- **Detect types** — unquoted logfmt and CSV values become JSON numbers,
  `true`/`false`, and `null`. Quoted values always stay strings, so
  `status="200"` survives as text. Values that would not round-trip — leading
  zeros like `007`, a leading `+` — stay strings too, keeping zip codes and phone
  numbers intact. Turn the option off to keep every value as a string.
- **Pretty-print** — indent the **JSON** target. NDJSON is always one compact
  record per line, so this option does not affect it.
- **Flatten** — when writing a flat format (logfmt or CSV), expand nested objects
  and arrays into dot-notation keys: `{"user":{"id":7}}` becomes `user.id=7` and
  a list becomes `tags.0`, `tags.1`. Turn it off to write nested values as
  compact JSON strings instead.
- **Keep only these fields** — a comma-separated allow-list that both selects and
  orders the output fields, e.g. `ts,level,msg`. Leave it blank to keep every
  field in first-seen order.

logfmt output follows the de-facto go-logfmt rules: `key=value` pairs separated
by a single space, one record per line, and values double-quoted whenever they
contain a space, an `=`, a quote, or a control character, with `\"`, `\\`, `\n`,
`\r`, and `\t` escapes.

### Worked example

Input logfmt (two records):

```
ts=2026-08-13T10:04:00Z level=info msg="user signed in" user_id=42 ok=true
ts=2026-08-13T10:04:07Z level=warn msg="slow query" dur_ms=812 ok=false
```

To **JSON** →

```
[{"ts":"2026-08-13T10:04:00Z","level":"info","msg":"user signed in","user_id":42,"ok":true},{"ts":"2026-08-13T10:04:07Z","level":"warn","msg":"slow query","dur_ms":812,"ok":false}]
```

To **CSV** → the header is the union of every record's keys, in first-seen order,
with a blank cell where a record has no such field:

```
ts,level,msg,user_id,ok,dur_ms
2026-08-13T10:04:00Z,info,user signed in,42,true,
2026-08-13T10:04:07Z,warn,slow query,,false,812
```

Going the other way, `[{"level":"error","msg":"disk full","retries":2}]` as
**JSON → logfmt** gives `level=error msg="disk full" retries=2` — the message is
quoted because it contains a space.

### Limits & edge cases

- Up to **1,000,000 characters** of input per run. Larger logs are rejected with
  a clear message rather than hanging the tab; split them first.
- Records must be objects. A JSON array of scalars (`[1,2,3]`) or a CSV file with
  only a header row is an error, not a silent empty result.
- A **bare logfmt key** with no `=` is treated as a boolean flag: `verbose msg=hi`
  parses to `{"verbose":true,"msg":"hi"}`.
- `null` and the empty string are kept apart when writing logfmt: a null writes as
  a bare `key=` and an empty string as `key=""`, so a JSON → logfmt → JSON round
  trip with **Detect types** on preserves the difference.
- Keys that logfmt cannot express — ones containing whitespace, `=`, or a quote —
  are sanitised to `_` (a `bad key` field becomes `bad_key`) rather than being
  dropped silently.
- If the same key appears twice on one logfmt line, the last value wins and the
  field keeps its first position.
- Blank lines are ignored, and a trailing `\r` from Windows line endings is
  stripped.

### Related conversions

This tool exists for the logfmt legs of the matrix — the JSON ↔ NDJSON ↔ CSV
legs work here too, and a dedicated tabular converter covers the CSV/TSV corners
(header-less rows, TSV) that log records do not need.

### FAQ

<details>
<summary>What exactly is logfmt?</summary>

logfmt is a plain-text structured logging convention: each line is a record made
of space-separated `key=value` pairs, like `level=info msg="user signed in"
user_id=42`. It is human-readable in a terminal and still machine-parseable,
which is why Heroku, Go services, and many Grafana/Loki pipelines emit it. There
is no formal specification — the Go `go-logfmt/logfmt` encoder is the de-facto
reference, and this tool follows its quoting and escaping rules.

</details>

<details>
<summary>Can it write logfmt, not just read it?</summary>

Yes — that is the point of this tool. Set **To** to **logfmt** and feed it JSON,
NDJSON, or CSV. Values get double-quoted only when they need it (a space, an
`=`, a quote, or a control character), and nested records are flattened into
dot-notation keys so the result stays a valid flat key space.

</details>

<details>
<summary>Why did a number come out as a string?</summary>

Three cases keep a value as text. Quoted logfmt values are always strings, so
`status="200"` stays `"200"`. Values with a leading zero (`007`) or a leading
`+` stay strings so zip codes, phone numbers, and product codes survive the round
trip. And with **Detect types** switched off, nothing is coerced at all.

</details>

<details>
<summary>My records have different fields — what happens in the CSV output?</summary>

The header row is the union of every record's keys in first-seen order, and rows
missing a key get a blank cell. No field is dropped and the columns stay aligned.
If you only want a fixed set of columns, list them in **Keep only these fields**
— that both filters and orders the output.

</details>

<details>
<summary>Is my log data uploaded anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely inside your
browser tab. Your log lines are never sent to a server, which matters because
logs routinely contain user IDs, tokens, and internal hostnames.

</details>
