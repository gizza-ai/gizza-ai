## About this tool

Log to Table extracts structured columns from semi-structured log lines. Use a custom Rust regular expression with named capture groups such as `(?P<level>\w+)`, or choose a preset for common formats. The named groups become output columns in a Markdown table, CSV, TSV, or JSON array.

Worked example:

Log input:

```text
ERROR 42 failed to connect
INFO 7 retry scheduled
```

Pattern:

```text
^(?P<level>\w+) (?P<code>\d+) (?P<message>.*)$
```

CSV output:

```csv
level,code,message
ERROR,42,failed to connect
INFO,7,retry scheduled
```

Presets cover Apache/nginx common and combined access logs, RFC 3164-style syslog, and log4j-style application lines. For other formats, switch to **custom** and provide the exact named-group regex.

## Limits and edge cases

- Custom patterns use Rust's linear-time `regex` engine, so backreferences and look-around are not supported.
- `limit` caps emitted rows from 1 to 5000; the default is 500.
- Blank input lines are ignored.
- Non-matching lines can be skipped, kept in an `unparsed` column, or treated as an error.
- CSV/TSV fields are quoted when needed; Markdown output escapes pipe characters inside cells.
- Multiline log entries are treated as separate lines; this tool does not merge stack traces into a single event.

## FAQ

<details>
<summary>How do I create columns with a custom pattern?</summary>

Use named capture groups. For example, `^(?P<ip>\S+) (?P<status>\d{3}) (?P<path>\S+)$` creates the columns `ip`, `status`, and `path` in that order.

</details>

<details>
<summary>What should I use for lines that do not match?</summary>

Use `skip` to drop them, `keep` to emit an `unparsed` column containing the raw line, or `error` when every line is expected to match and mismatches should fail fast.

</details>

<details>
<summary>Can this parse Apache or syslog without writing a regex?</summary>

Yes. Choose `common` or `combined` for Apache/nginx access logs, `syslog` for RFC 3164-style syslog, or `log4j` for typical timestamp/level/logger/message application logs.

</details>

<details>
<summary>Why does my regex work elsewhere but fail here?</summary>

This tool uses Rust regex syntax. It supports named captures with `(?P<name>...)` and avoids features such as look-around or backreferences to keep matching predictable and safe in WebAssembly.

</details>
