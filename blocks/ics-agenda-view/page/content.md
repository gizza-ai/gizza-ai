## About this tool

ICS Agenda View turns a pasted iCalendar (`.ics`) export into a readable agenda grouped by day. It is useful when you have a calendar export from a scheduling tool, ticketing system, conference schedule, or shared calendar and need a quick offline view of meetings plus the free gaps between them.

The parser handles unfolded and folded iCalendar lines, escaped text, all-day events, UTC timestamps, floating timestamps, many `TZID` values, `EXDATE`, and a bounded subset of common `RRULE` recurrences. Nothing is fetched from a calendar account: the pasted text is the entire input.

### Worked example

Paste this calendar:

```ics
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:a@example
DTSTART:20260309T090000Z
DTEND:20260309T093000Z
SUMMARY:Standup
LOCATION:Room 2
END:VEVENT
BEGIN:VEVENT
UID:b@example
DTSTART:20260309T110000Z
DTEND:20260309T120000Z
SUMMARY:Design review
END:VEVENT
END:VCALENDAR
```

With **Start date** set to `2026-03-09`, **Days** set to `1`, and the default `09:00`–`18:00` gap window, the output starts:

```text
Agenda 2026-03-09 to 2026-03-09 · UTC
Free gaps 09:00-18:00, at least 30m

Mon 2026-03-09
  09:00-09:30   Standup · Room 2
    free 1h 30m (09:30-11:00)
  11:00-12:00   Design review
    free 6h (12:00-18:00)
```

Switch **Output format** to JSON when you want machine-readable days, events, gaps, totals, and warnings.

### Useful controls

- **Start date** pins the window. Leave it blank to start at the earliest event.
- **Days** renders 1–90 days.
- **Display timezone** converts UTC and recognized event timezones into an IANA timezone such as `UTC`, `Europe/Berlin`, or `America/New_York`.
- **Gap window start/end** and **Minimum free gap** define which openings are reported.
- **Filter text** keeps matching events only, using summary, location, description, organizer, UID, and status.
- **Expand recurring events** expands common daily, weekly, monthly, and yearly recurrence rules inside the selected window.
- **Details** controls whether agenda lines include only summaries, locations, or full metadata.

### Limits and edge cases

- Input is capped at 1 MiB; recurrence expansion is capped at 5,000 occurrences and a fixed iteration budget.
- The window is capped at 90 days and minimum free gaps must be 5–480 minutes.
- Supported recurrences cover common `FREQ`, `INTERVAL`, `COUNT`, `UNTIL`, `BYDAY`, and `BYMONTHDAY` patterns. More exotic rules are listed once with a warning.
- Unknown `TZID` values fall back to the selected display timezone and produce a warning.
- Cancelled events are hidden by default. Turn on **Include cancelled events** when auditing schedule changes.
- Free-gap detection is interval math inside each day; it is not a multi-person availability merger.

## FAQ

<details>
<summary>Does this connect to Google Calendar, Outlook, or CalDAV?</summary>

No. It only parses pasted `.ics` text and never connects to an account, URL, or API. Export or copy the calendar data first, then paste it into the tool.

</details>

<details>
<summary>How are recurring events handled?</summary>

Common daily, weekly, monthly, and yearly `RRULE` patterns are expanded inside the selected window, and `EXDATE` exclusions are applied. Unsupported recurrence shapes are not guessed; the event is listed once and a warning explains the limitation.

</details>

<details>
<summary>Why does a timezone warning appear?</summary>

Some calendars use custom or Windows-style timezone names. The tool recognizes IANA zones and common Windows names. Unknown zones are interpreted in the selected display timezone so the agenda can still render, and the warning tells you that conversion was approximate.

</details>

<details>
<summary>Can this find free time across multiple calendars?</summary>

Not directly. This tool treats the pasted calendar as one schedule and reports gaps between its events. To compare several people, combine their busy events into one `.ics` first or use a dedicated free/busy merger.

</details>
