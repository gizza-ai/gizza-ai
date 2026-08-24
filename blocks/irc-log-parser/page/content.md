## About this tool

IRC clients all log the same conversation in stubbornly different ways. WeeChat writes
tab-separated columns, irssi writes a bare `21:07` and its own `-!-` event marker, mIRC and ZNC
bracket the time and say `*** Joins:`, HexChat writes `Jan 05 21:07:33` and never records the year.
This parser reads all of them, turns every line into a typed record, and gives you back one
consistent result: a readable timeline, or a JSON, NDJSON, CSV or Markdown export you can hand to a
script or a spreadsheet.

Everything happens on the text you paste. Nothing is uploaded, no files are read from disk, and
nothing is stored — so it works fine on a private channel log or an archive from a machine you no
longer have.

## Worked example

An irssi log, pasted as-is, with **Assume channel** set to `#gizza`:

```text
--- Log opened Fri Jan 05 20:00:00 2024
21:07 <alice> shipping the parser tonight
21:07 -!- bob [~bob@example.net] has joined #gizza
21:08  * alice waves
21:09 <bob> nice, I'll review it
21:10 -!- mode/#gizza [+o bob] by alice
21:11 -!- alice [~a@example.net] has quit [Ping timeout: 240 seconds]
```

With the default **Readable timeline** output and ISO timestamps:

```text
--- Log opened Fri Jan 05 20:00:00 2024
2024-01-05T21:07:00  <alice> shipping the parser tonight
2024-01-05T21:07:00  --> bob (~bob@example.net) joined #gizza
2024-01-05T21:08:00  * alice waves
2024-01-05T21:09:00  <bob> nice, I'll review it
2024-01-05T21:10:00  --  mode #gizza +o bob by alice
2024-01-05T21:11:00  <-- alice quit (Ping timeout: 240 seconds)
```

The log itself only carries `21:07`; the date comes from the `--- Log opened` marker, which the
parser reads and then rolls forward at every `--- Day changed` line.

Switch **Export as** to **CSV** and the same input becomes columns instead:

```text
line,time,type,nick,host,channel,arg,text
1,,meta,,,#gizza,,Log opened Fri Jan 05 20:00:00 2024
2,2024-01-05T21:07:00,message,alice,,#gizza,,shipping the parser tonight
3,2024-01-05T21:07:00,join,bob,~bob@example.net,#gizza,,
4,2024-01-05T21:08:00,action,alice,,#gizza,,waves
5,2024-01-05T21:09:00,message,bob,,#gizza,,"nice, I'll review it"
6,2024-01-05T21:10:00,mode,alice,,#gizza,+o bob,
7,2024-01-05T21:11:00,quit,alice,~a@example.net,#gizza,,Ping timeout: 240 seconds
```

## The record fields

Every record has the same eight fields, whichever client wrote the log, so a script never has to
care about the source format:

| Field | Meaning |
| --- | --- |
| `line` | 1-based line number in the pasted text, so you can go back to the source |
| `time` | The timestamp, rendered the way the **Timestamps** option asks for |
| `type` | `message`, `action`, `notice`, `join`, `part`, `quit`, `kick`, `nick`, `mode`, `topic`, `meta` or `unknown` |
| `nick` | Who acted — the speaker, the joiner, the person who was kicked |
| `host` | The `user@host` mask, when the log recorded one |
| `channel` | The channel the line belongs to, or the **Assume channel** value |
| `arg` | The second party or payload: the new nick, the kicker, or the mode string |
| `text` | The message, the action, the part/quit reason, or the new topic |

`arg` exists so that a kick keeps both people (`nick` was kicked, `arg` did the kicking) and a nick
change keeps both names, without inventing a different column layout per event type.

## Which format should I pick?

**Detect automatically** is right nearly always: it scores all six timestamp grammars over the
first 200 non-blank lines and takes the best fit. Pin one explicitly when a log is short, mixed, or
when auto-detection guesses wrong on an unusual client. The grammar only decides how the timestamp
is peeled off the front of each line — the event wording of every client (`-!-`, `***`, `-->`,
`Joins:`, `Parts:`, `was kicked`, `sets mode`) is understood in all modes, because clients freely
borrow each other's phrasing.

## Limits and edge cases

- Up to 5 MB of log text per run, and up to 200,000 records out. `limit` is applied *after* the
  include and nick filters; `0` means no limit.
- Lines that match no known IRC shape are kept as `unknown` records rather than dropped, so nothing
  disappears silently. If *no* line is recognised you get an error naming the format that was used.
- HexChat logs record a month and day but no year. The year comes from the surrounding
  `**** BEGIN LOGGING AT …` banner, or from **Base date** — without either, those lines carry a
  time only.
- `nick: message` lines are deliberately *not* read as messages. That is not IRC syntax, and
  guessing would turn any line containing a colon into a fake speaker.
- Nick prefixes (`@op`, `+voice`, `%halfop`) are stripped from the `nick` field. Turn on **Keep the
  original line** if you need them back.
- `+` is not treated as a channel prefix, so a mode string like `+m` is read as a mode and not as a
  channel name. Modeless `+channels` are not supported for that reason.
- Part and quit reasons are taken from the trailing `(…)` or `[…]` group. A reason that itself ends
  in a bracketed phrase keeps only that phrase.
- Encrypted, redacted or truncated lines are parsed as whatever they look like — the tool has no
  way to know a line was tampered with.

## FAQ

<details>
<summary>Which clients' logs does this understand?</summary>

The six timestamp grammars cover WeeChat (`2024-01-05 21:07:33` then tab-separated nick and text),
irssi (a bare `21:07` or `21:07:33`), mIRC, ZNC, EnergyMech and Textual (`[21:07:33]`), HexChat and
XChat (`Jan 05 21:07:33`), any log written with a plain ISO date and time, and logs with no
timestamps at all. On top of that, the event wording used by all of those clients is recognised in
every mode, so a log that has been passed through a converter or hand-edited usually still parses.

</details>

<details>
<summary>My log only has times, not dates — how do I get real timestamps?</summary>

Put the day the log covers into **Base date** as `YYYY-MM-DD` and every time-only line is dated
from it. If the log contains irssi's own `--- Log opened …` or `--- Day changed …` markers, those
are read automatically and take over from that point onward, which is what makes a multi-day irssi
log come out with the right date on each side of midnight. With neither, records keep a time and no
date, and the ISO output falls back to `21:07:00`.

</details>

<details>
<summary>How do I strip the join/part/quit noise?</summary>

Set **Include** to *Messages only*, which keeps message, action and notice lines and drops joins,
parts, quits, kicks, nick changes, mode and topic changes. The reverse — *Events only* — is useful
for auditing who was in a channel and when. To follow one person instead, put their nick in **Only
these nicks**; a trailing `*` matches by prefix, so `bob*` follows someone across `bob`, `bob_` and
`bobby`.

</details>

<details>
<summary>What is the difference between JSON and NDJSON here?</summary>

JSON gives you one pretty-printed array, which is what you want when a person or a browser is going
to read it. NDJSON puts one compact record on each line with no wrapping array, which is what
streaming tools expect: you can pipe it straight into `jq`, append two logs together with `cat`, or
read it line by line without holding the whole file in memory. The records themselves are
identical.

</details>

<details>
<summary>Why did my colour codes disappear?</summary>

**Strip colour and formatting codes** is on by default and removes the mIRC control characters —
bold, italic, underline, reverse, reset and the `^C` colour pairs — plus ANSI escape sequences,
because they turn into unreadable noise in a CSV cell or a JSON string. Turn it off to keep every
byte exactly as logged, or turn on **Keep the original line** to get the untouched line alongside
the cleaned fields.

</details>

<details>
<summary>Does it count who talked most or merge several log files?</summary>

No — this tool parses one pasted log into records and stops there, which keeps the output
predictable. Counting messages per person, activity by hour and word frequency is a separate job,
and so is stitching several days or several channels into one stream. Because every record keeps
its source `line` number and a normalised timestamp, the CSV or NDJSON output feeds straight into
whatever does the counting or merging.

</details>
