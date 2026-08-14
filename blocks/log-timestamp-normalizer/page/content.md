## About this tool

Every service writes its timestamps differently. The app logs ISO-8601, the load balancer
logs an Apache-style bracketed date, the container runtime logs raw epoch milliseconds and
`sshd` logs a syslog stamp with no year and no timezone at all. Reading an incident across
all of them means doing arithmetic in your head in four formats at once.

Paste the lines here and every timestamp comes back in **one** format and **one** timezone,
with the elapsed time since the previous event written next to it. The detection runs per
line, so a single paste can mix all of the formats above — you never have to tell the tool
what the input looks like.

It runs as WebAssembly inside this page: your logs stay in the browser tab. There is no
clock involved either — every instant comes out of the text you pasted, so the same paste
always produces the same output.

### A worked example

Paste this, leave every option alone:

```text
2023-12-01T10:15:30Z INFO  boot: starting service
1701425735123 WARN  cache: miss for key user:42
Dec  1 10:16:20 web01 nginx: GET /health 200
01/Dec/2023:10:17:05 +0000 "GET /api/orders HTTP/1.1" 200
1701425830 INFO  boot: ready
```

You get back:

```text
2023-12-01T10:15:30+00:00 INFO  boot: starting service  (start)
2023-12-01T10:15:35+00:00 WARN  cache: miss for key user:42  (+5.123s)
2023-12-01T10:16:20+00:00 web01 nginx: GET /health 200  (+44.877s)
2023-12-01T10:17:05+00:00 "GET /api/orders HTTP/1.1" 200  (+45s)
2023-12-01T10:17:10+00:00 INFO  boot: ready  (+5s)
```

Five lines, five different source formats, one output format. The syslog line on line 3
carries no year — it took 2023 from the dated lines around it. The `(+44.877s)` on that
same line is the gap this tool exists to make visible.

### Formats it detects

Each line is tried against the most specific pattern first, so an ISO stamp on a line that
also contains a ten-digit request id is read as the stamp, not the id.

| Source format | Looks like |
|---|---|
| ISO-8601 / RFC 3339 | `2023-12-01T10:15:30.123Z`, `2023-12-01 10:15:30`, `2023-12-01T11:15:30+01:00` |
| RFC 2822 / HTTP date | `Fri, 01 Dec 2023 10:15:30 +0000`, `Sun, 06 Nov 1994 08:49:37 GMT` |
| Apache / nginx | `10/Oct/2000:13:55:36 -0700` |
| Slashed date-time | `2023/12/01 10:15:30` |
| Syslog (RFC 3164) | `Dec  1 10:15:30` — no year, no zone |
| Epoch seconds / ms / µs / ns | `1701425730`, `1701425730123`, `1701425730123456`, `1701425730123456789` |
| Fractional epoch | `1701425730.25` |

The epoch unit comes from the digit count, which is why an 11-digit number is *not* read as
a timestamp: it is neither seconds nor milliseconds, so it is almost certainly an id.

### The two things a log timestamp can be missing

**A timezone.** A bare `2023-12-01 10:15:30`, an Apache stamp with no offset, and every
syslog line carry a wall-clock time with nothing saying which wall. Set **"Read zone-less
timestamps as"** to the zone the machine was configured for. Named zones are daylight-saving
correct per timestamp, from the bundled IANA database: `2023-07-01 12:00:00` read as
`Europe/Berlin` is 10:00 UTC, but `2023-12-01 12:00:00` is 11:00 UTC. Stamps that already
carry a zone or an offset ignore this setting entirely.

**A year.** Syslog stamps have a month and a day and nothing else. Leave the year at `0` and
it is inferred from the nearest line in the paste that does have one — trying that year, the
one before and the one after, and keeping whichever lands closest in time, so a log that runs
from December into January doesn't jump twelve months at the rollover. Set an explicit year
between 1970 and 2100 when the paste is syslog-only, otherwise those lines stay unmatched.

### Deltas and gaps

The delta annotation is the second half of the job. Each event gets the elapsed time since
the previous one in parentheses — `(+5.123s)`, with `(start)` on the first and a minus sign
on any step that goes backwards in time. Set **"Mark gaps at or above"** to a number of
seconds and every step at or over it gets ` GAP` appended, which turns "find the slow step"
into a text search. Turn on the summary header and you also get the format mix, the span from
first to last event, the largest gap and the input line it falls on:

```text
# 5 lines · 5 timestamps · 0 without one
# formats: apache 1, epoch-ms 1, epoch-s 1, iso-8601 1, syslog 1
# span: 2023-12-01T10:15:30+00:00 → 2023-12-01T10:17:10+00:00 (1m40s)
# largest gap: 45s at input line 4
# gaps at or above 30s: 2
# output: iso8601 in UTC
```

### Limits and edge cases

- Up to **50,000 lines** per run. A larger paste is refused rather than truncated.
- Lines with no timestamp — stack-trace frames, banners, wrapped JSON — are kept in place by
  default, and they stay attached to the event above them, so sorting can never tear a
  traceback away from its error.
- Bare `01/12/2023` dates are deliberately **not** detected. `MM/DD` and `DD/MM` are
  indistinguishable, and guessing wrong shifts an event by months without telling you.
- `epoch_seconds` and `epoch_millis` output is UTC by definition, so the output timezone has
  no effect on it.
- Only the first timestamp on a line is rewritten. A line carrying two stamps keeps the
  second one as it was written.

## FAQ

<details>
<summary>Why did my syslog lines come out unchanged?</summary>

A syslog stamp like `Dec  1 10:15:30` has no year in it, so the tool needs one from somewhere.
With the year left at `0` it borrows the year from the nearest line in the same paste that
carries a full date. If your paste is *only* syslog lines, there is nothing to borrow from and
those lines are left exactly as they were rather than being dated with a guess. Type the year
into the "Year for syslog stamps" box and they will normalize.

</details>

<details>
<summary>My times are off by an hour or two. What did I set wrong?</summary>

Almost always "Read zone-less timestamps as". A timestamp with no `Z`, no `+01:00` and no
offset says nothing about which timezone it was written in, so the tool has to be told —
and it defaults to UTC. Set it to the zone the machine that wrote the log was configured
for. If you set it to a named zone like `Europe/Berlin` rather than a fixed `+01:00`, summer
and winter timestamps are each converted with the offset that was actually in force on that
date, which a fixed offset gets wrong for half the year.

</details>

<details>
<summary>Can I merge logs from two different services into one timeline?</summary>

That is what the sort setting is for. Paste both logs one after the other, set the line order
to "Oldest event first", and the events interleave by their real instant regardless of which
format or zone each service wrote them in. Set "Each output line is" to "The normalized
timestamp, then the original line" so you can still tell which service a line came from.
Untimestamped lines travel with the event above them, so multi-line stack traces stay intact
through the sort.

</details>

<details>
<summary>How do I find the slow step in a boot or deploy log?</summary>

Leave the delta annotation on, then put a threshold in "Mark gaps at or above this many
seconds" — 60 for a boot or deploy, 0.5 for a request trace. Every step at or over it is
tagged with ` GAP`, so searching the output for `GAP` jumps you straight to the stalls. Turn
on the summary header too: it names the largest gap and the input line it falls on before you
scroll anywhere.

</details>

<details>
<summary>Why wasn't the number in my line treated as a timestamp?</summary>

Epoch values are recognised by digit count: 10 digits is seconds, 13 is milliseconds, 16 is
microseconds, 19 is nanoseconds, and `1701425730.25` is fractional seconds. Anything else —
an 11-digit number, a 12-digit number — is far more likely to be an order id or a trace id
than a time, so it is left alone. The same rule protects you the other way round: on a line
that has both an ISO timestamp and a ten-digit id, the ISO timestamp wins.

</details>

<details>
<summary>Is any of this uploaded?</summary>

No. The whole thing is a WebAssembly module running in this page, so your log text never
leaves the browser tab — which matters, because logs carry hostnames, user ids and request
paths. You can load the page, disconnect from the network and it still works. The same
conversion is available in the command-line tool if you would rather pipe a file through it.

</details>
