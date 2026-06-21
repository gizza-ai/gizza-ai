# extract-dates — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-dates` — scan text, find every date/time mention,
list them normalized to ISO 8601.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Python `dateparser` / `datefinder`, JS `chrono-node` | Library | Powerful NL date parsing, but require writing code and a runtime; not a paste-and-go tool. |
| Spreadsheet `DATEVALUE` / regex finds | App | Only handle one cell/format at a time; no normalization to a single ISO output; regex is brittle. |
| Online "find dates in text" tools | Web | Rare, often upload text to a server, and usually emit the raw matches without ISO normalization or validity checking. |
| Manual regex | DIY | Easy to write, hard to get right (validity, leap years, am/pm, ambiguity, overlap of date+time). |

## How gizza's tool is better / different

1. **Many formats → one canonical output.** ISO, numeric (`MM/DD/YYYY`),
   year-first, month-name (`January 5, 2024` / `5 Jan 2024`), and clock times
   (`14:30`, `3pm`, `9:05 AM`) all normalize to ISO 8601 (`YYYY-MM-DD`,
   `…THH:MM:SS`, `HH:MM:SS`).
2. **Validity-checked.** Backed by `chrono`, so impossible dates/times
   (`Feb 30`, `2024-13-40`, `25:99`) are dropped rather than emitted as garbage.
3. **Order-preserving, de-overlapped.** Results come back in document order, and
   a time embedded in an ISO datetime isn't double-counted as a separate time.
4. **Honest about ambiguity.** Numeric `01/05/2024` is read month-first (US)
   unless the first field can only be a day (>12) — documented, not silent.
5. **Runs locally, three surfaces.** Chat ("pull the dates out of this"), CLI
   (`gizza tool extract-dates`), and a zero-upload browser page — one Rust core.

## Verification

CLI run on *"We met on January 5, 2024 at 3pm, then again 2024-02-10 14:30 and
25/12/2024."* returned all four in order: `2024-01-05` (date), `15:00:00` (time),
`2024-02-10T14:30:00` (datetime), `2024-12-25` (date) — including the day-first
disambiguation of `25/12`.

## Scope / honest limitations

- No relative dates ("yesterday", "next Friday", "in 3 days") — those need a
  reference "now", which doesn't fit a pure recompute model. Could be a future
  opt-in with an explicit reference date.
- No timezone offset parsing yet (times are normalized as wall-clock).

## Possible future enhancements

- Day-first/month-first toggle for non-US users.
- Timezone-aware datetimes (parse `+01:00` / `Z`).
- Optional relative-date resolution given a reference date.
