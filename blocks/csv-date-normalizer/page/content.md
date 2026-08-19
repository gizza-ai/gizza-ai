## About this tool

A date column that came from more than one place is never one format. Somebody's export wrote `2021-06-01`, somebody's spreadsheet wrote `06/15/2021`, a hand-typed row says `15 Jan 2024`, one cell is the raw Excel serial `45000` because the sheet was saved as CSV, another is a Unix timestamp from an API, and one row just says `n/a`. Sort that column and it sorts as text. Load it into a database and half the rows are rejected. Chart it and the axis is nonsense.

The genuinely hard value is `03/04/2024`. Row by row it is unknowable — 4 March to most of the world, 3 April in the US — and a parser that guesses per cell will happily produce a column where some rows went one way and some the other, which is worse than not converting at all. This tool refuses to guess that way. It reads the **whole column first**, finds the rows that can only be read one way (any day above 12, like `25/12/2021`), and uses them to settle day-first vs month-first for every value in that column. If nothing in the column proves it, the tool says so instead of pretending; if the column proves *both* orders, it flags a conflict so you can fix the source rather than ship a silently mixed column.

Everything else is passed through untouched: other columns, the header row, the row order, the quoting and the delimiter. Values that cannot be read are never guessed at — keep them, blank them, or stop on the first one, whichever you choose.

### Worked example

Input, with every default (auto-detect the column, ISO 8601 output, auto day/month order):

```text
id,joined
1,2021-06-01
2,06/15/2021
3,15 Jan 2024
4,not a date
```

Output:

```text
id,joined
1,2021-06-01
2,2021-06-15
3,2024-01-15
4,not a date
```

The `id` column was left alone — auto-detection never claims a column of bare numbers, because a column of plain integers is an amount or an id far more often than a date. `not a date` was kept verbatim rather than turned into an empty cell or a wrong date.

Switch **Output** to the audit report and the same input explains itself:

```text
Normalized 1 column(s) to iso-auto across 4 data row(s).
Converted: 2   Unreadable: 1

Column "joined" (index 1) — day/month order month-first (detected)
  values 4, converted 2, already normalized 1, unreadable 1, blank 0
    row 4 (line 5): "not a date"
```

`06/15/2021` has a 15 in the second position, so it can only be month-first — that single row settled the order for the column, and the report says `detected` rather than `default`. The `already normalized 1` is `2021-06-01`, which was ISO before it arrived.

### What it can read

- **ISO 8601** — `2024-01-15`, `2024-01-15T10:30:00`, `2024-01-15T10:30:00Z`, `2024-01-15 10:30:00+05:30`, fractional seconds.
- **Numeric dates** with `/`, `-` or `.` separators, year first or year last, two- or four-digit years.
- **Written months** with or without ordinals and a leading weekday — `Jan 15th, 2024`, `15 January 2024`, `01-Jun-2021`, `Sept. 3 1999`, `Tue, 15 Jan 2024 10:30:00 +0000`.
- **12-hour times** — `3/4/2024 2:30 PM` — and trailing zone tokens `Z`, `UTC`, `GMT`, `+05:30`, `-0500`.
- **Compact digits** — `20240115` and `20240115103000`.
- **Unix epoch** seconds and milliseconds, recognised by magnitude, and **Excel 1900-system serials** (`45000` → `2023-03-15`, a fractional part becoming the time of day).

### Controls

- **Columns to normalize** — header names, 0-based indexes, or a mix (`start,2`), comma-separated. `auto` claims a column when at least 60% of its non-blank cells parse as a *written* date. Auto deliberately skips columns of bare numbers, so a Unix-timestamp or Excel-serial column must be named.
- **Output format** — ISO 8601 keeping each value's precision (default), ISO date only, ISO date-time, ISO shifted to UTC, Unix seconds or milliseconds, US `01/15/2024`, European `15/01/2024`, SQL `2024-01-15 10:30:00`, compact `20240115`, RFC 2822, or a custom strftime pattern such as `%d %B %Y` → `15 January 2024`.
- **Day/month order** — auto (infer per column, default), or force day-first or month-first. Values that settle themselves — ISO, written month names — ignore this setting entirely.
- **Two-digit year cut-off** — the pivot for `01/02/69`. At the default `68`, `68` becomes 2068 and `69` becomes 1969, the same rule POSIX `%y` and Excel use. Set it to `99` to push every two-digit year into the 2000s.
- **Read bare numbers as Excel serial dates** — on by default. Turn it off when a named column holds real numbers that must not be reinterpreted.
- **Unreadable values** — keep the original text (default), blank the cell, or stop on the first one with its row, line and column named.
- **First row is a header** — off means every row is data and columns must be given as indexes.
- **Delimiter** — `auto` (default) sniffs it from the first non-blank line; or `comma`, `tab`, `semicolon`, `pipe`, or any single character. The output uses the same separator.
- **Output** — the rewritten CSV (default), a plain-text audit report, or JSON with the audit plus the rewritten CSV under `csv`.

### Limits and edge cases

The table is capped at 5,000,000 bytes and the report lists the first 20 unreadable values per column. Impossible calendar dates are refused, not rolled over: `2021-02-30` and `2021-13-01` stay as they were rather than becoming 2 March and January 2022, while a real leap day like `2020-02-29` converts normally. A trailing token the parser does not recognise (an unknown zone abbreviation such as `EST`, say) makes the whole value unreadable rather than being silently dropped — named US/European zone abbreviations are ambiguous across regions, so a numeric offset is the only thing accepted. Only the named columns are rewritten; the tool does not sort, dedupe, re-type or reformat anything else, and it never uses your computer's clock or time zone — every value comes from the cell itself. Everything runs locally in your browser and the table is never uploaded.

## FAQ

<details>
<summary>Is 03/04/2024 read as 4 March or 3 April?</summary>

Whichever the rest of the column proves. The tool scans every value in the column before rewriting anything and looks for rows that can only be read one way — a first part above 12 like `25/12/2021` proves day-first, a second part above 12 like `06/15/2021` proves month-first. That verdict is then applied to every ambiguous value in the same column, so a column is never half one convention and half the other.

If the column contains no such proof, the tool falls back to month-first and the audit report marks the reason as `default` rather than `detected` — that is your signal to set **Day/month order** explicitly. If the column proves *both* orders, the report says `conflict` and prints a warning line: the source data is genuinely mixed, and no setting can rescue it without you deciding what the ambiguous rows mean.

</details>

<details>
<summary>Why did auto-detection skip my timestamp column?</summary>

On purpose. `1700000000` is a valid Unix timestamp and `45000` is a valid Excel serial, but a column of bare integers is an amount, a quantity or an id far more often than it is a date — and converting a price column into dates is a much worse outcome than not converting a timestamp column. Auto-detection only claims columns whose cells look like *written* dates.

Name the column instead: type its header (or its 0-based index) into **Columns to normalize** and the numbers are read as timestamps or serials. If a named column holds numbers you do *not* want treated as spreadsheet dates, turn off **Read bare numbers as Excel serial dates** — Unix epochs are still recognised by magnitude, since no plausible Excel serial reaches 100,000,000.

</details>

<details>
<summary>What happens to a value the parser cannot read?</summary>

Never a guess. Under the default **Unreadable values** setting the original text stays exactly where it was, and the value is counted and listed in the audit report with its row number and its line number in the file (they differ whenever a quoted field spans lines). Choose *Blank the cell* to empty it instead — useful when the destination wants a real null — or *Stop and report the first one* to abort the run with the offending row, line and column named, which is the right setting for an import you would rather fail loudly than half-convert.

Blank cells are always left blank under every setting, and are counted separately from unreadable ones.

</details>

<details>
<summary>Does it change my time zone or use my computer's clock?</summary>

No. Nothing in the tool reads your clock or your locale. A value with no offset is treated as a plain wall-clock time and stays that way; a value that states an offset keeps it under the ISO formats and is shifted only when you explicitly ask for **ISO shifted to UTC**, **Unix seconds** or **Unix milliseconds**, which are absolute instants and therefore have to resolve the offset. A value with no offset at all is treated as UTC by those three formats, because there is no other defensible choice — if that matters, normalize to `iso-datetime` and attach the zone downstream.

</details>

<details>
<summary>How do two-digit years work, and can I control the century?</summary>

`01/02/69` has to mean either 1969 or 2069, and the answer is a policy, not a fact. **Two-digit year cut-off** is that policy: a two-digit year at or below the pivot lands in the 2000s, above it in the 1900s. The default `68` matches POSIX `strftime %y` and Excel, so `68` → 2068 and `69` → 1969.

Set it to `99` if your data is all recent (every two-digit year becomes 20xx) or to `0` if it is all historical (every one becomes 19xx). Four-digit years are always taken literally and the pivot never touches them.

</details>

<details>
<summary>Can I get something other than ISO — Unix time, SQL, or my own layout?</summary>

Yes. Besides the four ISO variants there are Unix epoch seconds and milliseconds, US `01/15/2024`, European `15/01/2024`, SQL `2024-01-15 10:30:00`, compact `20240115` and RFC 2822. For anything else pick **Custom** and give a strftime pattern in **Custom pattern**: `%d %B %Y` → `15 January 2024`, `%b %-d, %Y` → `Jan 15, 2024`, `%Y/%m/%d %H:%M` → `2024/01/15 10:30`. An unknown specifier is rejected with a message rather than emitted as literal text, so a typo can't quietly corrupt a whole column.

Note that the default **ISO 8601, keep precision** is per value, not per column: a date-only cell stays date-only and a cell that carried a time keeps it. Pick **ISO date only** if you want every row to have exactly the same shape.

</details>
