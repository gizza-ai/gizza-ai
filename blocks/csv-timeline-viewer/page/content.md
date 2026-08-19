## About this tool

Paste a CSV, TSV, or JSON Lines export of timestamped events — an application log, an audit trail,
an access log, a support-ticket dump — and slice it without opening a spreadsheet. The timestamp
column is found for you, the time range is inclusive on both ends, the search box looks in every
column at once, and the row numbers in the output always point back at the source file.

Everything runs locally in your browser. The data you paste is never uploaded.

### Worked example

Input, with the column filter `level == ERROR`:

```csv
timestamp,level,service,message
2024-06-01T10:00:01Z,INFO,api,request started
2024-06-01T10:00:05Z,ERROR,api,upstream timeout
2024-06-01T10:00:09Z,WARN,worker,retrying job 42
2024-06-02T11:30:00Z,ERROR,worker,job 42 failed
```

Output:

```text
#  timestamp             level  service  message
-  --------------------  -----  -------  ----------------
2  2024-06-01T10:00:05Z  ERROR  api      upstream timeout
4  2024-06-02T11:30:00Z  ERROR  worker   job 42 failed

showing rows 1-2 of 2 matched (4 read) | time column: timestamp | span: 2024-06-01T10:00:05Z .. 2024-06-02T11:30:00Z
```

The leading `#` column is each row's position in the file you pasted, so a row you find after
sorting and filtering can still be located in the original. The footer states how many rows
matched and how many were read, so a page is never mistaken for the whole result.

### What it understands

Timestamps: ISO 8601 / RFC 3339 (`2024-06-01T10:00:05Z`, with or without an offset),
`2024-06-01 10:00:05`, `2024/06/01 10:00:05`, `06/01/2024 10:00:05`, `01-Jun-2024 10:00:05`,
Apache/nginx `01/Jun/2024:10:00:00 +0000`, plain dates, and bare epoch values in seconds,
milliseconds, microseconds, or nanoseconds (chosen by magnitude).

Filters: one `<column> <op> <value>` condition per line, all of which must hold, with `op` one of
`==` `!=` `<` `<=` `>` `>=` `contains` `!contains` `startswith` `endswith` `matches`. Comparison is
numeric when both sides are numbers, otherwise text. `matches` takes a regular expression.

### Limits and edge cases

Input is capped at 200,000 lines and 200,000 data rows; `limit` caps one page at 100,000 rows.
In the aligned table, cells longer than 60 characters are truncated with `…` and embedded newlines
are shown as spaces — switch the output to CSV, JSON, or JSON Lines to get the untouched values.
Rows whose timestamp cannot be parsed are kept when no time range is set, sorted after every dated
row, and dropped (with a count in the footer) when `from` or `to` is used. Ragged rows are padded
with empty cells rather than rejected. There is no wall clock here, so relative ranges such as
"last hour" are not available — give explicit `from` / `to` values, which also makes a shared link
reproduce the same result later.

## FAQ

<details>
<summary>How does it decide which column holds the time?</summary>

First it looks for a header that names a time — `timestamp`, `time`, `date`, `datetime`, `ts`,
`created_at`, `event_time`, `TimeCreated`, `@timestamp` and similar, ignoring case, spaces and
punctuation — and checks that the column's values actually parse. If no header matches, it falls
back to the first column where at least half of the values parse as timestamps. Set `time_column`
explicitly (a header name or a 1-based index) when a file has several date columns and you want a
specific one.

</details>

<details>
<summary>My timestamps have no timezone. Which one is assumed?</summary>

Values that already carry a `Z` or a `±hh:mm` offset are used as-is. Values without one are read as
UTC unless you set `tz_offset`, which says how many hours the data is ahead of UTC — `-5` for US
Eastern standard time, `5.5` for India. The same offset is applied to `from` and `to` when those
have no timezone either, so a range you type in local time selects the rows you expect.

</details>

<details>
<summary>Can I search only some columns, or use a regular expression?</summary>

Yes. Leave `search_fields` blank to search every column, or list the ones you want by name or
1-based index (`message, service`). Search is a case-insensitive substring by default; tick the
regex option to treat the search text as a regular expression such as `job \d+`, and tick match
case to compare exactly. An invalid pattern is reported as an error that names the syntax problem
rather than silently matching nothing.

</details>

<details>
<summary>How do I page through a big result?</summary>

`limit` is the page size and `offset` is how many matching rows to skip, so `limit=100` with
`offset=100` is the second page. The footer always reports the total number of matches, not just
the size of the page, so you can tell how many pages there are. Sorting and filtering happen before
paging, so pages stay consistent as you move through them.

</details>

<details>
<summary>What does the activity summary show?</summary>

Choosing the summary output replaces the rows with statistics over the whole match set, ignoring
`limit` and `offset`: how many rows were read and matched, the column list, the detected timestamp
column, the first and last matching event, and a histogram that buckets matches over time (from one
second up to a year per bucket, chosen so the chart stays about sixty rows tall). It is the quickest
way to see when a burst of errors actually started.

</details>

<details>
<summary>Does it accept JSON Lines and headerless files?</summary>

Both. JSON Lines input takes one JSON object per line — a whole JSON array of objects works too —
and the columns are the union of every object's keys, in first-seen order, with missing values left
blank. For a delimited file with no header row, turn the header option off; columns are then named
`column1`, `column2`, and so on, and you can also reference them by 1-based index anywhere a column
name is accepted.

</details>
