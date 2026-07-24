## About this tool

**ICS to CSV** turns an iCalendar file — the `.ics` format that Google Calendar,
Apple Calendar, Outlook and event invites export — into a clean CSV table you can
open in a spreadsheet or import anywhere. Paste the file contents and you get one
row per event with these columns:

`summary, start, end, location, description`

plus **status**, **categories** and **uid** columns whenever your events carry
them, so the CSV never has a column of blanks. Folded lines are unfolded, RFC 5545
text escapes (`\n`, `\,`, `\;`) are decoded back to real characters, alarm and
timezone sub-blocks are skipped, and every field is RFC-4180 escaped so summaries
or descriptions containing commas, quotes or newlines stay intact.

Everything runs locally in your browser — the calendar never leaves your device.

### Worked example

Input (one event):

```text
BEGIN:VCALENDAR
BEGIN:VEVENT
UID:evt-1@example.com
SUMMARY:Team Standup
DTSTART:20240309T081530Z
DTEND:20240309T083000Z
LOCATION:Room 4
DESCRIPTION:Daily sync
END:VEVENT
END:VCALENDAR
```

Output (defaults — comma delimiter, header on, ISO dates):

```csv
summary,start,end,location,description,uid
Team Standup,2024-03-09T08:15:30Z,2024-03-09T08:30:00Z,Room 4,Daily sync,evt-1@example.com
```

### Options

- **Delimiter** — comma, semicolon (handy for comma-decimal locales), tab (TSV),
  or pipe.
- **Include header row** — turn the column-name row off for a headerless CSV.
- **Date column format** — `iso` normalizes to ISO-8601
  (`20240309T081530Z` → `2024-03-09T08:15:30Z`, `20240704` → `2024-07-04`), `raw`
  keeps the original `.ics` value text, and `unix` converts to epoch seconds.
- **Include location / description** — drop either column when you don't need it
  (descriptions can be long).

### Limits & edge cases

- Only `VEVENT` blocks become rows. `VTODO`, `VJOURNAL` and `VFREEBUSY` are
  ignored, and `VALARM` / `VTIMEZONE` sub-blocks inside an event are skipped so
  their fields never leak into the event row.
- All-day events use a date-only `DTSTART` / `DTEND`; the `DTEND` of an all-day
  event is the exclusive end date per the iCalendar spec — it is copied through as
  written, not adjusted.
- `unix` epoch treats `Z` (UTC) times exactly; floating times and `TZID` times are
  read as wall-clock UTC because the tool ships no timezone/DST database. Use `raw`
  or `iso` to keep the original wall-clock value untouched.
- Recurrence (`RRULE`) is not expanded — a repeating event is one row for its
  master `DTSTART`, not one row per occurrence.
- A date value the parser can't read is passed through unchanged rather than
  failing the whole conversion.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>What columns does the CSV have?</summary>

Always `summary`, `start`, and `end`, then `location` and `description` unless you
turn those off. `status`, `categories` and `uid` columns are added automatically
whenever at least one event includes them, so you never get a column that is
entirely empty. An empty cell means that event didn't record that value.

</details>

<details>
<summary>Does it handle all-day events and different date formats?</summary>

Yes. Date-only values like `DTSTART;VALUE=DATE:20240704` are recognized as all-day
events and written as `2024-07-04`. Timed values are normalized to ISO-8601, or you
can switch the date format to `raw` to keep the original `.ics` text or `unix` for
epoch seconds. `Z`-suffixed times are UTC; floating and `TZID` times are read as
wall-clock UTC.

</details>

<details>
<summary>Why did some events not turn into rows?</summary>

Only calendar events (`VEVENT` blocks) are exported. To-dos (`VTODO`), journal
entries (`VJOURNAL`) and free/busy blocks (`VFREEBUSY`) are skipped, and a
repeating event appears once for its master start date — recurrences (`RRULE`) are
not expanded into individual occurrences.

</details>

<details>
<summary>Is my calendar file uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using WebAssembly — the `.ics`
text you paste is processed on your own device and is never sent to a server.

</details>
