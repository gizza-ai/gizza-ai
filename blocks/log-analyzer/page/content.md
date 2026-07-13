## What this tool does

Paste a chunk of raw logs and this tool reads every line, works out the format,
and gives you a one-screen **summary** instead of a wall of text. It runs entirely
in your browser — nothing is uploaded, it works offline once the page has loaded,
and there is no sign-up.

You get four things:

- **Counts by level** — every entry mapped to a unified severity
  (trace / debug / info / warn / error) with its share of the total.
- **Top errors** — the most frequent warning and error messages, ranked by count.
  Near-identical messages are **grouped**: numbers and long hex/UUID ids are masked,
  so `db timeout attempt 3` and `db timeout attempt 5` collapse into
  `db timeout attempt #` with a count of 2.
- **Time span** — the first and last timestamps found.
- **Volume timeline** — entries bucketed by minute, hour, or day (picked
  automatically from the span) with a little bar chart.

It auto-detects and analyzes five common shapes:

| Format | Looks like |
| --- | --- |
| **JSON / NDJSON** | one JSON object per line (`level`/`msg`/`ts` keys) |
| **logfmt** | `level=info msg="hi" ms=12` |
| **Syslog** | `<34>Oct 11 22:14:15 host su: ...` (RFC 3164) or `<PRI>1 ...` (RFC 5424) |
| **Access log — Common (CLF)** | `ip - user [date] "req" status size` |
| **Access log — Combined** | Common + `"referer" "user-agent"` |

For access logs there is no `level` field, so severity comes from the HTTP status:
**5xx → error**, **4xx → warn**, everything else → info. Any line the parser can't
match is still counted (as an info-level entry) so the totals stay honest.

Want the row-by-row table, filtering, or CSV export instead of a summary? Use the
sibling **Log Parser** tool — this one is the aggregate view.

## Worked example

Paste these JSON logs with **Format: Auto-detect**:

```
{"ts":"2024-01-01T00:00:00Z","level":"info","msg":"server started"}
{"ts":"2024-01-01T00:20:00Z","level":"warn","msg":"high latency 900ms"}
{"ts":"2024-01-01T00:40:00Z","level":"error","msg":"db timeout attempt 3"}
{"ts":"2024-01-01T00:41:00Z","level":"error","msg":"db timeout attempt 5"}
```

The summary opens with a caption, breaks the four entries down by level, groups the
two `db timeout attempt N` errors into one row, and buckets the volume per minute:

```
json · 4 entries · 2024-01-01 00:00:00 → 2024-01-01 00:41:00

## Levels

| level | count | share |
| --- | --- | --- |
| error | 2 | 50.0% |
| warn | 1 | 25.0% |
| info | 1 | 25.0% |

## Top errors

| count | level | message |
| --- | --- | --- |
| 2 | error | db timeout attempt # |
| 1 | warn | high latency #ms |
```

Switch **Output as** to **JSON object** for the same data as a structured object you
can feed into a script.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>How is this different from the Log Parser tool?</summary>

**Log Parser** gives you the row-by-row view — every line pulled apart into named
fields, with severity/text filtering and Markdown / JSON / CSV export. **Log
Analyzer** (this tool) skips the rows and gives you the *aggregate*: how many entries
at each level, which errors happen most, over what time span, and the volume over
time. Use the parser to inspect lines; use the analyzer to spot the trends.

</details>

<details>
<summary>How are "top errors" grouped?</summary>

Only warning- and error-level entries are ranked. Before counting, each message is
normalized: runs of digits and long hex / UUID strings are replaced with `#`, and
whitespace is collapsed. That way `timeout on host db-01` and `timeout on host db-02`
count as the same recurring error rather than two one-offs. The list is capped by the
**Top errors to list** setting (default 10, up to 100).

</details>

<details>
<summary>What timestamp formats does it understand?</summary>

ISO 8601 / RFC 3339 (`2024-01-01T00:00:00Z` and offsets), plain
`YYYY-MM-DD HH:MM:SS`, unix epoch seconds or milliseconds, the Apache/nginx
`10/Oct/2000:13:55:36 -0700` access-log date, and the syslog `Oct 11 22:14:15`
form (no year — pinned to a reference year so ordering and bucketing still work).
Entries with no recognizable timestamp are counted and reported separately; if
nothing has a timestamp the timeline is skipped.

</details>

<details>
<summary>Is my log data uploaded anywhere?</summary>

No. The analysis runs as WebAssembly inside your browser tab. Your logs never leave
your machine, there is no server round-trip, and the page keeps working offline once
it has loaded.

</details>

<details>
<summary>Why does the volume timeline pick minutes for one log and days for another?</summary>

The **Timeline granularity** is set to **Auto** by default: spans up to a couple of
hours bucket per minute, up to a couple of days per hour, and longer spans per day —
so the chart always has a readable number of bars. Force **Per minute**, **Per hour**,
or **Per day** if you want a fixed granularity.

</details>
