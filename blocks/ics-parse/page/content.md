## About this tool

**ICS Parser** turns an iCalendar (`.ics`) file into a clean, structured JSON
array — one object per event — so you can inspect a calendar, feed it to an API,
or load it into a script without hand-writing an RFC 5545 parser. Paste the file
contents and every `BEGIN:VEVENT…END:VEVENT` block becomes an event object, in
document order.

Each event carries its `uid`, `summary`, `start` and `end` times, an `all_day`
flag (set when the date has no time component or is tagged `VALUE=DATE`),
`location`, `description`, `status`, and `categories` as a list. People are
parsed too: `organizer` and each `attendee` become a `name` + `email` pair, read
from the `CN` parameter and the `mailto:` value. Empty or absent fields are left
out so the JSON stays tidy — no columns of blanks, no `null`s.

Repeating events are decoded into a structured **`recurrence`** object rather than
left as a raw `RRULE` string: `freq`, `interval`, `count`, `until`, and day
patterns like `byday` each become their own field, with numbers where the value
is numeric and arrays where it is a comma list.

Choose how dates are written with **Date format**: `ISO-8601` normalizes
`20240309T081530Z` to `2024-03-09T08:15:30Z`, `raw` keeps the original `.ics`
text, and `Unix` converts to epoch seconds. Toggle **Pretty-print** for indented
versus compact JSON, and turn off **Include description** to drop long
description fields. Everything runs locally in your browser — your calendar is
never uploaded.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>What fields does each event include?</summary>

Every event object can carry `uid`, `summary`, `start`, `end`, `all_day`,
`location`, `description`, `status`, `categories`, `organizer`, `attendees`, and
`recurrence`. A field is only emitted when the source event actually has it, so
an event with no location or attendees simply omits those keys instead of showing
blanks or `null`.

</details>

<details>
<summary>How is a recurring event (RRULE) represented?</summary>

The `RRULE` is parsed into a structured `recurrence` object. For example
`FREQ=WEEKLY;INTERVAL=2;COUNT=10;BYDAY=MO,WE,FR` becomes
`{"freq":"WEEKLY","interval":2,"count":10,"byday":["MO","WE","FR"]}` — numeric
parts become numbers, comma lists become arrays, and an `UNTIL` value is
formatted with the same **Date format** setting as the event's start and end.

</details>

<details>
<summary>Are timezones converted?</summary>

Times marked with a trailing `Z` (UTC) are handled exactly. Floating times and
times with a `TZID` parameter are read as **wall-clock UTC** — no timezone
database is shipped in the browser sandbox, so the tool does not shift a
`TZID`-tagged time into a target zone. If you need true timezone conversion, pick
`raw` to keep the original value and convert it downstream.

</details>

<details>
<summary>Which calendar components are parsed?</summary>

Only `VEVENT` components are parsed. task, journal, and free-busy components are
ignored, and nested sub-components inside an event — such as `VALARM` reminders
and `VTIMEZONE` definitions — are skipped so their properties never leak into the
event object.

</details>

<details>
<summary>Is my calendar uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely inside your browser
tab. The `.ics` text you paste never leaves your device — there is no server call
and nothing is stored, so it is safe to use with private or work calendars.

</details>
