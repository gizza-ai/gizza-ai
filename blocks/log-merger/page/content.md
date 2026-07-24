## Merge log files into one timeline

When an incident spans several services, the answer is usually hidden in the
*ordering* of events across logs — the request that arrived, the job that fired,
the error that followed. **Log Merger** interleaves two or more pasted logs into
a single stream sorted by each line's timestamp, with every line tagged by the
source it came from, so you can read the whole story top to bottom.

Everything runs locally in your browser. Nothing is uploaded, so you can safely
paste production logs.

### How it works

1. Paste your logs into one box. Mark where each source begins with a header
   line — `--- api.log ---`, `=== worker ===`, GNU `tail`'s `==> api.log <==`,
   or a markdown `# api.log`. (No headers handy? Switch **How to split sources**
   to *Blank-line-separated blocks* and the tool names them `source1`, `source2`,
   … in order.)
2. Every line's timestamp is detected automatically — ISO 8601 / RFC 3339,
   `YYYY-MM-DD HH:MM:SS`, syslog `Mon DD HH:MM:SS`, Apache/nginx
   `10/Oct/2000:13:55:36 -0700`, and bare unix epoch seconds or milliseconds.
3. Lines are merged and sorted; each keeps a `[source]` tag so you never lose
   track of where an event came from.

### Worked example

Paste an API log and a worker log delimited by headers:

```
--- api.log ---
2024-06-01T10:00:02Z GET /users 200
2024-06-01T10:00:05Z GET /orders 200
--- worker.log ---
2024-06-01T10:00:01Z job started
2024-06-01T10:00:04Z job done
```

With **Oldest first** and aligned tags on, you get one timeline:

```
[worker.log] 2024-06-01T10:00:01Z job started
[api.log]    2024-06-01T10:00:02Z GET /users 200
[worker.log] 2024-06-01T10:00:04Z job done
[api.log]    2024-06-01T10:00:05Z GET /orders 200
```

The worker started one second before the first API request — obvious now that
the two logs sit on one clock.

### Limits & edge cases

- Each source needs at least one line with a recognizable timestamp; if the tool
  finds none anywhere it tells you rather than guessing an order.
- A line with **no** timestamp inherits the previous line's timestamp within its
  source, so stack traces and wrapped messages stay attached to the entry above
  them. Leading untimestamped lines borrow their source's first timestamp.
- Syslog timestamps carry no year, so they're pinned to a fixed reference year —
  relative ordering within a capture is correct, but don't mix syslog lines from
  different years and expect them to sort by real date.
- All timestamps are compared in UTC; an offset in the line (e.g. `+02:00` or an
  Apache `-0700`) is honored, but a bare local time with no offset is read as-is.
- Input is capped at 50,000 lines total. Ties (identical timestamps) keep their
  original source-then-line order.

## FAQ

<details>
<summary>How do I tell the tool where each log starts?</summary>

Put a header line above each source. Recognized headers are `--- name ---`,
`=== name ===`, GNU `tail`'s `==> name <==` banner (so you can paste the output
of `tail -f a.log b.log` straight in), and a markdown `# name`. The `name`
becomes the `[source]` tag. If you'd rather not add headers, switch **How to
split sources** to *Blank-line-separated blocks* and separate each log with one
blank line.

</details>

<details>
<summary>Which timestamp formats are recognized?</summary>

ISO 8601 / RFC 3339 (`2024-06-01T10:00:02Z`, with or without the `T`, fractional
seconds, and zone), `YYYY-MM-DD HH:MM:SS`, syslog `Mon DD HH:MM:SS`, Apache/nginx
`10/Oct/2000:13:55:36 -0700`, and bare unix epoch seconds or milliseconds as the
first token. The timestamp can appear anywhere in the line — it doesn't have to
be at the start.

</details>

<details>
<summary>What happens to lines with no timestamp, like stack traces?</summary>

They inherit the timestamp of the previous line in the same source, so a
multi-line error and its stack frames stay glued together in the merged output
instead of scattering to the top. Untimestamped lines before a source's first
timestamp borrow that first timestamp.

</details>

<details>
<summary>Is anything uploaded to a server?</summary>

No. The merge runs entirely in your browser via WebAssembly — your logs never
leave your machine, which is why it's safe to paste production output.

</details>

<details>
<summary>The two captures overlap and I see duplicate lines. Can I remove them?</summary>

Yes — turn on **Drop duplicate lines**. When a later line repeats a
`(timestamp, text)` pair that's already been emitted, it's skipped, which is
handy when two captures cover the same window.

</details>
