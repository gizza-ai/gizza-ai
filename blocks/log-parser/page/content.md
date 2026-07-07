## What this tool does

Paste a chunk of raw logs and this tool figures out the format, pulls each line
apart into named fields, and shows them as a structured, filterable table. It
runs entirely in your browser — nothing is uploaded, it works offline once the
page has loaded, and there is no sign-up.

It auto-detects and parses five common shapes:

| Format | Looks like | Fields you get |
| --- | --- | --- |
| **JSON / NDJSON** | one JSON object per line | every key in the object, in order |
| **logfmt** | `level=info msg="hi" ms=12` | one column per key |
| **Syslog** | `<34>Oct 11 22:14:15 host su: ...` (RFC 3164) or `<PRI>1 ...` (RFC 5424) | `timestamp`, `host`, `tag`/`app`, `pid`, `message` |
| **Access log — Common (CLF)** | `ip - user [date] "req" status size` | `ip`, `user`, `time`, `request`, `status`, `size` |
| **Access log — Combined** | Common + `"referer" "user-agent"` | the Common fields plus `referer`, `user_agent` |

Anything the chosen parser can't match is kept as a single `message` field, so no
line is silently dropped.

## Filtering

- **Minimum severity** — one filter that works across every format. Severity is
  unified: a JSON/logfmt `level` (or `lvl`/`severity`) key, a syslog priority, or
  an HTTP status (5xx → error, 4xx → warn, otherwise info). Picking **Warning and
  up** keeps warnings *and* errors.
- **Filter text** — keep only lines containing your text (case-insensitive). Tick
  **regular expression** to match a regex against the raw line instead.
- **Max rows** — cap how many rows are rendered (default 200, up to 5000) so a big
  paste stays snappy.

## Output

Choose **Markdown table** (the default, with a stats caption like
`combined · 3 entries · 1 error · 1 warn`), **JSON array** (one object per line,
key order preserved), or **CSV** (header + rows, ready for a spreadsheet).

## Worked example

Paste this combined access log with **Format: Auto-detect**:

```
127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.0" 200 2326 "-" "Mozilla/5.0"
10.0.0.5 - - [10/Oct/2000:13:55:37 -0700] "GET /missing HTTP/1.1" 404 512 "-" "curl/7.0"
10.0.0.9 - - [10/Oct/2000:13:55:38 -0700] "POST /api HTTP/1.1" 500 12 "-" "Go-http-client/1.1"
```

The tool detects **combined**, maps the `500` to *error* and the `404` to *warn*,
and renders:

```
combined · 3 entries · 1 error · 1 warn

| ip | ident | user | time | request | status | size | referer | user_agent |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 127.0.0.1 | - | - | 10/Oct/2000:13:55:36 -0700 | GET /index.html HTTP/1.0 | 200 | 2326 | - | Mozilla/5.0 |
| 10.0.0.5 | - | - | 10/Oct/2000:13:55:37 -0700 | GET /missing HTTP/1.1 | 404 | 512 | - | curl/7.0 |
| 10.0.0.9 | - | - | 10/Oct/2000:13:55:38 -0700 | POST /api HTTP/1.1 | 500 | 12 | - | Go-http-client/1.1 |
```

Switch **Minimum severity** to *Error and up* and only the `500` row remains.

## Limits and edge cases

- Output is a single text result, not an infinite-scroll viewer, so it caps at
  **5000 rows** (200 by default). Paste a slice of a huge file, or raise the cap.
- **Auto-detect** takes a majority vote over the first lines; a file that mixes
  formats parses best if you pick the format explicitly.
- **Timestamps are kept as text** exactly as they appear — they are not
  re-parsed or converted to a common zone.
- Syslog severity needs a `<priority>` prefix; a plain BSD line without one is
  treated as *info*.

## FAQ

<details>
<summary>Is anything uploaded to a server?</summary>

No. All parsing happens locally in your browser with WebAssembly, so your logs
never leave your device and the tool works offline once loaded.

</details>

<details>
<summary>How does the severity filter work across different formats?</summary>

Every line is mapped to one unified severity — **Trace, Debug, Info, Warn, or
Error** — from whatever the format provides: a JSON/logfmt `level` key, a syslog
priority number, or an HTTP status code (`5xx` → error, `4xx` → warn). The
**Minimum severity** control is a threshold, so *Warning and up* keeps both
warnings and errors.

</details>

<details>
<summary>What's the difference between Common and Combined access logs?</summary>

**Common Log Format (CLF)** ends at the response size. **Combined** adds two more
quoted fields — the `Referer` and the `User-Agent` — which is the default for
Apache and nginx. Auto-detect tries combined first and falls back to common.

</details>

<details>
<summary>Can I export the parsed logs?</summary>

Yes. Set **Output as** to **JSON array** for one object per line (key order
preserved) or **CSV** for a spreadsheet-ready file. Both respect the active
severity and text filters, and there is a copy button on the result.

</details>

<details>
<summary>My log format isn't listed — what happens?</summary>

Lines the selected parser can't match are still shown, with the whole line in a
single `message` column, so nothing disappears. If auto-detect can't recognise
anything it asks you to pick a format. For CSV data, use the CSV tools instead.

</details>
