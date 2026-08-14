## About this tool

Calendars import `.ics` files; spreadsheets export CSV. This tool is the missing step
between them. Paste a table of events — one row per event, with a header row naming the
columns — and it hands back a standards-compliant iCalendar document you can save as
`schedule.ics` and import into Google Calendar, Outlook, Apple Calendar, Thunderbird or
anything else that speaks iCalendar.

The conversion runs as WebAssembly inside this page, so your schedule never leaves the
browser tab. The output is deterministic too: the same CSV always produces the same bytes,
which keeps re-imports and version-controlled calendar files clean.

### A worked example

Paste this, leave every option alone:

```csv
title,start,end,location
Team sync,2026-07-24 09:00,2026-07-24 09:30,Room 2
Conference,2026-07-27,2026-07-29,Berlin
```

You get back:

```text
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//gizza-ai//csv-to-ics//EN
CALSCALE:GREGORIAN
BEGIN:VEVENT
UID:team-sync@1.local
DTSTAMP:19700101T000000Z
DTSTART:20260724T090000
DTEND:20260724T093000
SUMMARY:Team sync
LOCATION:Room 2
END:VEVENT
BEGIN:VEVENT
UID:conference@2.local
DTSTAMP:19700101T000000Z
DTSTART;VALUE=DATE:20260727
DTEND;VALUE=DATE:20260730
SUMMARY:Conference
LOCATION:Berlin
END:VEVENT
END:VCALENDAR
```

The first row had a time, so it became a half-hour timed event. The second had bare dates,
so it became a three-day all-day event — note the `DTEND` of the 30th, because iCalendar
all-day ends are exclusive and the tool adds that day for you.

### Columns it recognises

Header names are matched without regard to case, spaces, hyphens or underscores, so
`Start Date`, `start_date` and `START-DATE` are the same column. Anything unrecognised is
ignored rather than rejected.

- **Title** (required) — `title`, `summary`, `name` or `event`
- **Start** (required) — `start`, `start_date`, `begins` or `date`
- **End** — `end`, `end_date` or `ends`
- **Duration** — `duration_minutes` or `duration`, in whole minutes
- **Description** — `description`, `details` or `notes`
- **Location** — `location` or `place`
- **UID** — `uid` or `id`
- **All-day** — `all_day` or `all-day`, true for `true`, `yes`, `y`, `1`, `x` or `t`

### Dates, times and lengths

Dates are written as `2026-07-24`, `2026-07-24 09:00` or `2026-07-24T09:00`, with optional
seconds. A start with no time — or a truthy all-day cell — makes an all-day event written as
`VALUE=DATE`. Anything else is a timed event, and it ends at its `end` cell, else its
`duration_minutes` cell, else the default event length (60 minutes unless you move the
slider).

The timezone choice decides how timed events are anchored. **Floating** writes
`DTSTART:20260724T090000` with no zone marker, so 09:00 shows as 09:00 wherever the event is
opened — the right choice for a class timetable or a personal schedule. **UTC** treats the
pasted times as already being UTC and writes them with the trailing `Z`, so each calendar
converts them into its own local time — the right choice for a release plan or a call across
several countries. All-day events carry no time either way.

### Limits and edge cases

- Quoted cells are read as one value, so `"Lunch, with team"` keeps its comma, and commas,
  semicolons, backslashes and newlines are escaped in the output the way RFC 5545 requires.
- Lines longer than 75 octets are folded with a leading space on the continuation, so long
  descriptions import intact instead of being truncated by a strict parser.
- Up to **5000 rows** per run. Rows that are entirely blank are skipped.
- A row with an unreadable date, a duration that is not a whole number of minutes, a missing
  title or start, or an end that falls before its start is reported by row number — the file
  is not written with a broken event in it.
- Event IDs come from the `uid` column when you have one. Without it, an ID is generated from
  the title and the row's position, which stays stable across runs, so re-importing the same
  file updates the events instead of duplicating them.

## FAQ

<details>
<summary>How do I get the .ics file into Google Calendar?</summary>

Copy the output into a text editor and save it with an `.ics` extension, then open Google
Calendar on the web, go to Settings → Import & export, choose the file and pick the calendar
to add the events to. Outlook uses File → Open & Export → Import/Export → Import an iCalendar
(.ics) file, and Apple Calendar imports with File → Import. Import into a new, empty calendar
the first time — that way you can delete the whole calendar in one click if the mapping was
not what you wanted.

</details>

<details>
<summary>Why is my event showing up as all-day when I wanted a time?</summary>

The start cell had no time in it. `2026-07-24` is a date, so it becomes an all-day event;
`2026-07-24 09:00` is a moment, so it becomes a timed one. Spreadsheets often strip the time
from a column formatted as a date — check the exported CSV in a plain text editor rather than
in the spreadsheet. If your times live in a separate column, join them into the start column
first (`=A2&" "&TEXT(B2,"HH:MM")` in most spreadsheets).

</details>

<details>
<summary>My multi-day event is one day short. What happened?</summary>

Nothing — check the dates in your calendar rather than in the `.ics` text. iCalendar all-day
ends are *exclusive*, so an event running the 27th through the 29th is written as `DTEND` on
the 30th. This tool adds that day for you, which means you should enter the last day the
event actually runs, not the day after it.

</details>

<details>
<summary>Should I pick floating or UTC?</summary>

Pick floating when everyone attends in their own local time — a school timetable, a
conference agenda handed to attendees who are all on site, a personal routine. 09:00 then
means 09:00 on whatever clock the reader has. Pick UTC when the events are fixed instants
that people join from different countries — a release window, a global standup — and paste
times you have already converted to UTC, so each calendar shows the correct local time.
There is no per-row timezone column: convert the times before pasting if the rows are mixed.

</details>

<details>
<summary>Can I add reminders and descriptions?</summary>

Yes. Add a `description`, `details` or `notes` column and its text lands in each event's
`DESCRIPTION`, and a `location` or `place` column fills `LOCATION`. Turn on "Add a 15-minute
reminder" to attach a pop-up alarm to every event. That reminder is all-or-nothing and fixed
at 15 minutes before the start; for an all-day event it fires 15 minutes before midnight, so
leave it off for holiday or deadline calendars.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The conversion is a WebAssembly module running in this page, so your event list stays in
the browser tab. You can load the page, disconnect from the network and it still works. The
same conversion is available offline in the command-line tool if you would rather script it.

</details>
