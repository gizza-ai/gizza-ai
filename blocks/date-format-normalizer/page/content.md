## About this tool

Real text almost never uses one date format. A document that passed through a spreadsheet, an
export, an email thread and someone's handwriting ends up with `03/04/2024`, `15 April 2024`,
`2024-04-22` and `5.1.24` sitting in the same paragraph. This tool finds every one of them and
rewrites them into the single format you pick, leaving the rest of the text byte-for-byte
untouched.

The interesting part is not the rendering — it is deciding what `03/04/2024` means. Instead of
guessing per date, the tool reads the whole text first and lets the dates that can only be one
thing settle it for the rest: a single `15/04/2024` in the paste proves the source writes
day-first, so `03/04/2024` is 3 April, not 4 March. When nothing in the text settles it, the
month-first reading is used and the **report** mode tells you exactly which dates were guessed.

### Worked example

Input:

```
Order 4471 was placed 03/04/2024 and shipped 15/04/2024.
The invoice is dated 22 April 2024 and falls due 2024-05-06.
Support ticket opened Friday, 3 May 2024 at 2:30 PM.
```

Output with the default ISO 8601 format:

```
Order 4471 was placed 2024-04-03 and shipped 2024-04-15.
The invoice is dated 2024-04-22 and falls due 2024-05-06.
Support ticket opened 2024-05-03T14:30.
```

Three source forms, one target form. `15/04/2024` settled the day-first reading for
`03/04/2024`; the weekday name was absorbed into the date it belonged to; the clock time came
along with its date; and the sentence's full stop survived.

Switch the format to **Day first** with a **dot** separator and the same text renders
`03.04.2024`, `15.04.2024`, `22.04.2024`, `06.05.2024`. Switch to **Year first** with the
separator set to **none** and `scan 5 Jan 2024.pdf` becomes `scan 20240105.pdf` — the stamp that
makes a folder of files sort itself.

### What it recognizes

| Form | Examples |
| --- | --- |
| ISO 8601 | `2024-01-05`, `2024-1-5`, `2024-01-05T14:30:00Z`, `2024-01-05 14:30` |
| Numeric | `01/05/2024`, `5.1.2024`, `5-1-24`, `2024/01/05` |
| Month name | `January 5, 2024`, `Jan. 5th, 2024`, `5 Jan 2024`, `5th of January 2024` |
| With a weekday | `Friday, 5 January 2024`, `Fri, 05 Jan 2024 14:30:00 +0100` |
| Unix epoch | `1704465000`, `1704465000123` — only when you switch timestamp detection on |

A clock time written next to a date is picked up with it, including the `at 2:30 PM` phrasing and
a trailing `Z`, `UTC` or `+01:00`.

### Output formats

ISO 8601 · year-first, day-first and month-first numeric (with your choice of dash, slash, dot,
space or no separator) · full or abbreviated month names in either order · RFC 2822 email dates ·
Unix seconds or milliseconds · or any chrono/strftime pattern you want, such as `%B %-d, %Y` or
`%Y%m%d`.

Three ways to get the result back: the **text** with the dates rewritten in place, a **list** of
just the normalized values one per line for pasting into a spreadsheet column, or a **report**
giving each date's line and column, what was found, what it became, and whether the day/month
order had to be guessed.

### Limits and edge cases

- Up to **1,000,000 bytes** of text per run. Split anything larger and run the parts separately.
- Strings that look like dates but are not — `2024-02-30`, `13/13/2024`, a version number like
  `1.2.2024` where no valid reading exists — are left exactly as written, never guessed at.
- Bare 10- and 13-digit numbers are only read as timestamps when you ask for it, and then only
  when they land between 1973 and 2100. Order numbers and phone numbers stay untouched.
- Two-digit years need a century: the pivot decides it, defaulting to the POSIX rule where 68 and
  below mean 20xx.
- The timezone setting moves only the dates that carry an explicit offset. A date written with no
  zone has nothing to convert from, so it stays where it is.
- Everything runs in your browser. No upload, no account, no network round-trip, and the same
  input always produces the same output.

## FAQ

<details>
<summary>How does it decide whether 03/04/2024 is 3 April or 4 March?</summary>

It reads the whole text before rewriting anything. Any numeric date with a field above 12 can
only be read one way — `15/04/2024` must be day-first, `04/15/2024` must be month-first — and
those dates set the reading for every ambiguous date in the same text. If the text contains no
such date, or contradicts itself, the month-first reading is used and the **report** output mode
flags every date that was guessed, so you can check them instead of trusting them. You can also
skip the inference entirely by setting the reading to **Day first** or **Month first** yourself.

</details>

<details>
<summary>Does it change anything other than the dates?</summary>

No. In **text** mode the output is the input with each detected date span swapped for its
rewritten form; every other character, including line breaks and punctuation, is passed through
untouched. Two things are deliberately absorbed into the date they belong to: a weekday name
written in front of it (`Friday, 5 January 2024` becomes one date) and a clock time written after
it. A sentence-ending full stop after `2:30 pm.` is kept — only the `pm` is treated as part of
the time.

</details>

<details>
<summary>What happens to times and timezones?</summary>

A time found next to a date travels with it and is rendered on the 24-hour or 12-hour clock, with
seconds shown only when the source had them. Turn **Keep the clock time** off to reduce
everything to bare dates. The timezone field moves the dates that carry an explicit offset — an
ISO stamp ending in `Z` or `+01:00`, an RFC 2822 date, a detected epoch value — into UTC, an IANA
zone such as `Europe/Berlin` with daylight saving applied per date, or a fixed offset. That can
change the calendar day, which is the point. Dates written without any zone are left exactly
where they are.

</details>

<details>
<summary>Can I get a format that is not in the dropdown?</summary>

Yes — choose the **custom** format and write a strftime pattern. `%d.%m.%Y` gives `05.01.2024`,
`%B %-d, %Y` gives `January 5, 2024`, `%Y%m%d` gives `20240105`, `%A %d %b %y` gives
`Friday 05 Jan 24`. The common fields are `%Y`/`%y` for the year, `%m`/`%d` for month and day
(`%-m`/`%-d` without the leading zero), `%B`/`%b` for month names, `%A` for the weekday,
`%H:%M:%S` for a 24-hour clock, `%I:%M %p` for a 12-hour one and `%z` for the offset. An invalid
pattern is reported rather than half-rendered.

</details>

<details>
<summary>Why are my order numbers not being converted?</summary>

Because timestamp detection is off by default. Most long numbers in real text are order ids,
account references or phone numbers, and turning every 10-digit number into a date would ruin
more documents than it fixes. Switch **Also read bare 10- and 13-digit numbers as unix
timestamps** on when you are working with log exports or API payloads where epoch values really
are dates — and note that even then only values between 1973 and 2100 are accepted.

</details>

<details>
<summary>How do I check what it actually did?</summary>

Set the output to **report**. You get a `#` header with how many dates were found, the mix of
source forms detected, and which day/month order was chosen and why, followed by one
tab-separated line per date: its line and column in the source, the original string, the
rewritten value, and an `ambiguous` marker when the day/month order had to be inferred. It pastes
straight into a spreadsheet if you want to review a large document column by column.

</details>
