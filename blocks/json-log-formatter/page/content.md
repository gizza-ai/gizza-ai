## About this tool

`json-log-formatter` is for NDJSON or JSON Lines logs: one JSON object per line. It turns dense structured logs into an aligned view such as `[time] LEVEL message key=value`, or exports the same filtered records as a Markdown table, JSON array, or CSV.

It auto-detects common timestamp, level and message keys (`time`, `ts`, `@timestamp`, `level`, `severity`, `msg`, `message`) and can flatten nested context into dotted fields like `req.method` and `user.id`. Use the field filter to keep only records where a path contains or exactly equals a value, or leave the field blank to search the whole record.

The level filter understands words (`info`, `warning`, `error`, `critical`) plus common numeric conventions: bunyan/pino-style `10` through `60`, and syslog priorities `7` through `0`. Unknown custom level words still render; they sort like `info` for minimum-level filtering.

### Limits and edge cases

- Input must be line-delimited JSON objects. A JSON array line is invalid for this tool.
- Blank lines and lines starting with `#` or `//` are skipped.
- `limit` renders at most 5,000 records after filtering; the default is 200.
- Invalid JSON lines can be skipped, kept as raw message lines, or treated as errors with line numbers.
- This is not a full jq expression engine; use `field`, `filter`, `match`, and `fields` for focused log triage.

## FAQ

<details>
<summary>What log formats does this accept?</summary>

It accepts NDJSON/JSONL: one JSON object per line. That is the common output from structured loggers. It does not parse syslog text, logfmt, Apache logs, or a single JSON array document.

</details>

<details>
<summary>How do I filter for only errors?</summary>

Set `level` to `error`. The tool keeps records whose detected level is error or fatal. Numeric levels are mapped automatically for common bunyan/pino and syslog conventions.

</details>

<details>
<summary>Can I filter a nested field?</summary>

Yes. Keep `flatten` enabled and set `field` to a dotted path such as `req.method`, `user.id`, or `items.0.status`. Use `match=contains` for case-insensitive substring search or `match=exact` for an exact value.

</details>

<details>
<summary>What happens to bad lines in a mixed log file?</summary>

The default `on_invalid=skip` skips them and adds a count notice to text outputs. Choose `keep` when you want raw non-JSON lines to remain visible, or `error` when a malformed line should fail the run and report its line number.

</details>
