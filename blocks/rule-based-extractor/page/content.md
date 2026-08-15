## Extract structured fields with rules

Rule-based extraction is for the messy middle ground between a one-off regex and
a full parser. Paste text, write one rule per field, and get back structured
JSON, CSV, a readable listing, or a rule report. It is useful for logs, invoice
snippets, support emails, scraped pages, clipboard dumps, and any text where the
same date, code, email, IP address, ticket ID, or amount appears in a predictable
shape.

Each rule is one line. The most common form is `field = regex`, where the field
name becomes the output key and the first capture group (or the whole match if
there is no group) becomes the value. You can also write a bare regex with named
groups, or use Grok-style shortcuts such as `%{DATE_ISO:date}` and
`%{IPV4:client}` to keep patterns short. Define your own reusable macro with
`@NAME = regex`, then use `%{NAME}` or `%{NAME:field}` later in the rule block.

### Worked example

Input text:

```
Invoice INV-2026-014 dated 2026-08-15, billed to ada@example.com, total $1,240.00 (VAT 21%).
```

Rules:

```
invoice = INV-\d{4}-\d+
date = %{DATE_ISO}
email = %{EMAIL}
total = %{MONEY}
vat = %{PERCENT}
```

With **Output** set to JSON, the extractor returns:

```
{"invoice":"INV-2026-014","date":"2026-08-15","email":"ada@example.com","total":"$1,240.00","vat":"21%"}
```

For logs, switch **Split input into records by** to **Lines** so each line
becomes one output object. For every email address or every URL in a record,
switch **Matches per rule** to **Every match**; the JSON value becomes an array.

### Rule syntax

- `field = regex` extracts one named field. If the regex has a capture group,
  the first group is used; otherwise the full match is used.
- `%{PATTERN}` inserts a built-in macro as a non-capturing regex fragment.
- `%{PATTERN:field}` inserts a built-in macro as a named capture group.
- Bare regex lines must contain named captures, for example
  `%{DATE_ISO:date} %{LOGLEVEL:level} (?<message>.*)`.
- `@NAME = regex` defines a reusable macro for later lines.
- Blank lines, `# comments`, and `// comments` are ignored.

Built-ins include `WORD`, `INT`, `NUMBER`, `EMAIL`, `URL`, `IPV4`, `IPV6`,
`MAC`, `UUID`, `HASH`, `DATE_ISO`, `DATE_US`, `TIME`, `TIMESTAMP_ISO`,
`SYSLOG_TIME`, `LOGLEVEL`, `HTTP_METHOD`, `HTTP_STATUS`, `HOSTNAME`, `PATH`,
`MONEY`, `PERCENT`, `PHONE`, `SEMVER`, and `TICKET`.

### Options

- **Split input into records by** — run rules against the whole input, each
  line, each blank-line-separated paragraph, or chunks split by your own regex.
- **Matches per rule** — keep the first match per record, or collect every match
  as an array.
- **Ignore case**, **Multiline**, and **Dot matches newline** map to the common
  regex flags for case-insensitive, per-line anchors, and `.` crossing line
  breaks.
- **Trim captured values** removes surrounding whitespace from captures.
- **Drop duplicate values** deduplicates arrays when matching every occurrence.
- **When a rule matches nothing** can skip the key, keep it as `null` / empty, or
  fail fast with the field name.
- **Rule report** output shows how many records each rule hit, which is the
  fastest way to debug a pattern set before exporting JSON or CSV.

### Limits and edge cases

- Input text is capped at **1 MB**.
- At most **200 extraction rules** are accepted.
- `max_records` must be between **1 and 50,000**; `max_matches` must be between
  **1 and 10,000** per rule per record.
- Regex compilation uses Rust's regex engine: no look-around or backreferences,
  but matching is guaranteed not to blow up exponentially.
- If no rule matches anywhere, extraction fails with a hint to switch to the
  report output.
- Duplicate field names are rejected so a later rule cannot silently overwrite
  an earlier field.

## FAQ

<details>
<summary>How is this different from a normal regex tester?</summary>

A regex tester tells you whether a pattern matches. This tool turns a whole set
of named patterns into structured output. You can split text into records,
extract several fields per record, choose JSON or CSV, and use the report view to
see which rules never matched.

</details>

<details>
<summary>Can I use one rule block for many log lines?</summary>

Yes. Set **Split input into records by** to **Lines**. Each line is processed as
one record and the JSON output becomes an array of objects. Turn on **Skip records
with no matches** to ignore headers, blank lines, or unrelated lines.

</details>

<details>
<summary>What does `%{DATE_ISO:date}` mean?</summary>

`DATE_ISO` is a built-in pattern for dates such as `2026-08-15`, and `:date`
names the capture group. `%{DATE_ISO:date}` is equivalent to writing a named
regex group by hand, but it is shorter and less error-prone.

</details>

<details>
<summary>Why did my lookbehind or backreference fail?</summary>

The tool uses Rust's regex engine, which deliberately excludes look-around and
backreferences so matching remains predictable and fast in WebAssembly. Rewrite
the pattern with explicit captures, split the input into smaller records, or use
a separate parser if you need context-sensitive matching.

</details>

<details>
<summary>How do I debug a rule that is not matching?</summary>

Set **Output** to **Rule report**. The report lists every field, the rule line
that produced it, how many records it hit, and which fields never matched. That
lets you adjust one pattern without guessing from an empty JSON result.

</details>
