# Unix Timestamp Converter

Convert Unix epoch timestamps into readable UTC dates, or parse common date strings back into epoch values. The tool auto-detects the direction by default: a numeric input becomes a date, while a date/time string becomes timestamp values.

## What it supports

- Unix timestamps in **seconds**, **milliseconds**, **microseconds** or **nanoseconds**.
- Automatic unit detection by timestamp magnitude, with a manual unit override when you need it.
- ISO 8601 / RFC 3339 dates such as `2023-11-14T22:13:20Z` or `2023-11-15T00:13:20+02:00`.
- RFC 2822 email dates, slash dates, dotted dates and month-name dates through the shared date parser.
- UTC output including ISO 8601, RFC 2822, calendar components, day-of-year and ISO week.
- Date-to-timestamp output in all four units at once: seconds, milliseconds, microseconds and nanoseconds.

## Modes

- **auto** — default. Numeric input is treated as a timestamp; non-numeric input is parsed as a date.
- **to-date** — force timestamp-to-date conversion.
- **to-timestamp** — force date-to-timestamp conversion.

When parsing a wall-clock date with no explicit timezone, the tool assumes UTC and sets `assumed_utc: true` in the output. If the input carries an offset, the instant is shifted to UTC before timestamps are returned.

## Examples

- `1700000000` → `2023-11-14 22:13:20 UTC`
- `1700000000000` → auto-detected as milliseconds and converted to the same instant.
- `2023-11-15T00:13:20+02:00` → `1700000000` seconds.
- `January 1, 1970` → `0` seconds (assumed UTC midnight).

Everything runs locally in your browser or the gizza CLI; your dates never leave your device.
