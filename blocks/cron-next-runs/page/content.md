## What this cron tool does

Paste a **cron expression** and this tool shows you a plain-English description of the schedule plus the next times it will fire — entirely in your browser, so nothing you type leaves your machine. It's the quickest way to sanity-check a `crontab` line, a scheduled job, or a CI trigger before you ship it.

All times are computed and shown in **UTC**.

## Supported syntax

The tool understands standard **5-field** crontab syntax:

```
┌───────────── minute (0-59)
│ ┌─────────── hour (0-23)
│ │ ┌───────── day of month (1-31)
│ │ │ ┌─────── month (1-12 or JAN-DEC)
│ │ │ │ ┌───── day of week (0-7 or SUN-SAT; 0 and 7 are Sunday)
│ │ │ │ │
* * * * *
```

Each field accepts:

- `*` — every value
- a single number, e.g. `5`
- ranges `A-B`, e.g. `1-5`
- steps `*/S` and `A-B/S`, e.g. `*/15`, `0-30/10`
- lists `A,B,C`, e.g. `0,15,30,45`
- three-letter **month names** (`JAN`-`DEC`) and **weekday names** (`SUN`-`SAT`)

You can also add a **leading seconds field** (6 fields total), e.g. `*/30 * * * * *` runs every 30 seconds.

When both day-of-month and day-of-week are restricted, a time matches if **either** matches — the classic Vixie-cron behavior.

## Shortcuts

`@yearly` (`@annually`), `@monthly`, `@weekly`, `@daily` (`@midnight`) and `@hourly` are all accepted.

## Examples

- `*/15 * * * *` — every 15 minutes.
- `0 9 * * MON-FRI` — 09:00 on weekdays.
- `0 0 1 * *` — midnight on the first of every month.
- `0 0 13 * FRI` — every Friday **and** the 13th of each month, at midnight.
- `0 0 29 2 *` — only on a leap-year February 29th.

## Options

- **After** — leave blank to start from now, or give an ISO-8601 / RFC-3339 UTC timestamp (or a Unix epoch second) to preview the schedule from any point in time.
- **How many runs** — 1 to 100 upcoming times (default 5).
- **Output format** — **text** for an aligned, readable list, or **json** for a machine-readable object (`{ iso, epoch, weekday }` per run) you can pipe into other tooling.
