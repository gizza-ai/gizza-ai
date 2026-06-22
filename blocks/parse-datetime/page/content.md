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
