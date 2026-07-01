## About this tool

The date/time parser takes a single date or time string written in almost any
common format and breaks it into its individual parts — so you can read off the
year, month, day, weekday, hour, minute, second, day-of-year and ISO week
number, and get back a clean, canonical **ISO 8601** value you can paste
anywhere. Everything runs locally in your browser; the text you paste is never
uploaded.

## Formats it understands

- **ISO 8601 / RFC 3339** — `2024-01-05`, `2024-01-05T14:30:00`,
  `2024-01-05T14:30:00Z`, `2024-01-05T14:30:00+02:00`, and the space-separated
  `2024-01-05 14:30`.
- **RFC 2822 email dates** — `Fri, 05 Jan 2024 14:30:00 +0000` (the form found
  in email headers).
- **US slash dates** — `01/05/2024`, `1/5/24`. Read **month-first** (US style)
  unless the first field is greater than 12, in which case it is read
  day-first. A two-digit year maps `00–69` to the 2000s and `70–99` to the
  1900s.
- **European dotted dates** — `05.01.2024`, read **day-first**.
- **Year-first** — `2024/01/05`.
- **Month-name dates** — `January 5, 2024`, `5 Jan 2024`, `Jan 5 2024`, and the
  same with a trailing time such as `Jan 5, 2024 2:30 PM`.
- **Bare clock times** — `14:30`, `2:30:15`, `3pm`, `9:05 AM` (returned without
  a date).

## What you get back

For a date or datetime the tool reports the **kind** (date, time, or datetime),
the **year**, **month** (number and English name), **day**, **weekday** name,
**day-of-year** and **ISO week** number. For a time or datetime it adds the
**hour**, **minute** and **second** in 24-hour form, and the **UTC offset**
when the input carried a timezone. Every result includes a normalized
**ISO 8601** string. Invalid calendar dates (like February 30) and
unrecognizable input are reported as errors rather than guessed at.

## Privacy

This tool runs entirely client-side as WebAssembly. The string you paste stays
in your browser and is never sent to a server.

## FAQ

<details>
<summary>Is 01/05/2024 read as January 5 or May 1?</summary>

Slash dates are read **month-first** (US style), so `01/05/2024` is January 5.
The only exception is when the first field can't be a month — `25/12/2024` is
parsed day-first because 25 > 12. If your date is European-style with an
ambiguous first field, write it with dots (`05.01.2024`), which is always read
**day-first**, or use the unambiguous ISO form `2024-01-05`.

</details>

<details>
<summary>How are two-digit years interpreted?</summary>

`00–69` maps to the 2000s and `70–99` to the 1900s, so `1/5/24` means 2024 and
`1/5/85` means 1985. If that guess isn't what you want, spell out the full
four-digit year.

</details>

<details>
<summary>Does it convert between timezones?</summary>

No — it's a parser, not a converter. When the input carries an offset (like
`+02:00` or the `+0000` in an email date) the tool reports it and includes it in
the canonical ISO 8601 output, but it never shifts the clock time to another
zone.

</details>

<details>
<summary>What happens with an impossible date like February 30?</summary>

You get an error. Invalid calendar dates, out-of-range times, and strings the
parser can't recognize are reported as errors rather than silently "corrected"
to the nearest valid value.

</details>
