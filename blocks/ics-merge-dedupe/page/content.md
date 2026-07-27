## About this tool

**ICS Merge & Dedupe** combines one or more iCalendar files — the `.ics` format
that Google Calendar, Apple Calendar and Outlook export — into a single calendar
and removes duplicate events along the way. Paste the contents of one file, or
concatenate several files one after another, and you get back one clean
`BEGIN:VCALENDAR…END:VCALENDAR` document with each event kept only once.

Duplicates are matched the way real calendars overlap: by **UID** when both copies
carry one (the same event re-exported from the same source), or by **start time and
title** for the same public event that two different apps each stamped with their
own UID. Every surviving event is copied through **verbatim** — its properties,
folded lines, alarms and RFC 5545 escapes are untouched — so the result imports
cleanly back into any calendar app. Line endings are normalized to CRLF, identical
`VTIMEZONE` blocks are emitted once by their `TZID`, and `VTODO` / `VJOURNAL` /
`VFREEBUSY` components pass through unchanged.

Everything runs locally in your browser — your calendar never leaves your device.

### Worked example

Input — two files that both contain the same event (same `UID`):

```text
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:evt-1@example.com
DTSTART:20240309T081530Z
SUMMARY:Team Standup
END:VEVENT
END:VCALENDAR
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:evt-1@example.com
DTSTART:20240309T081530Z
SUMMARY:Team Standup
END:VEVENT
END:VCALENDAR
```

Output (defaults — smart matching, keep first, sorted) — one calendar, one event:

```text
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//gizza-ai//ics-merge-dedupe//EN
CALSCALE:GREGORIAN
BEGIN:VEVENT
UID:evt-1@example.com
DTSTART:20240309T081530Z
SUMMARY:Team Standup
END:VEVENT
END:VCALENDAR
```

### Options

- **Match duplicates by** — `smart` (default) uses the UID when an event has one and
  otherwise falls back to normalized start + title; `uid_start` matches UID *and*
  start time, so recurrence overrides that share a UID but differ in start are kept;
  `start_title` ignores the UID entirely and matches on normalized start + title,
  collapsing the same event exported by different apps.
- **Keep which copy** — when a duplicate group is found, `first` keeps the earliest
  occurrence in document order, while `last_modified` keeps the copy with the newest
  `LAST-MODIFIED` (falling back to `DTSTAMP`) so a later edit wins.
- **Sort events by start time** — on by default; turn it off to keep events in the
  order they appeared across the input files.
- **Calendar name** — optionally sets `X-WR-CALNAME`, the title Google/Apple/Outlook
  show for the merged calendar.

### Limits & edge cases

- Only `VEVENT` blocks are deduplicated. `VTODO`, `VJOURNAL` and `VFREEBUSY` are
  passed through unchanged (never merged), and at least one event is required.
- Title matching is whitespace-normalized and case-insensitive, so `Independence
  Day` and `Independence  Day` match — but genuinely different wording will not.
- Start times are compared after normalizing, so `20240309T081530Z` and its ISO
  form compare equal. `Z` (UTC) times are exact; floating and `TZID` times are read
  as wall-clock UTC because no timezone/DST database is shipped — the same wall-clock
  value from two files still matches.
- Recurrence rules (`RRULE`) are **not** expanded — a repeating event stays a single
  master event, and `uid_start` will not treat its instances as separate.
- Events with no UID, start *and* title at all carry no identity and are always
  kept, never merged away.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>How does it decide two events are the same?</summary>

It depends on the **Match duplicates by** option. With `smart` (the default), two
events match if they share a `UID`; an event with no UID falls back to matching on
its normalized start time and title. `uid_start` additionally requires the start
times to match, which keeps recurrence overrides that reuse one UID. `start_title`
ignores UIDs completely and matches on start + title, which is what collapses the
same public event that Google and Apple each exported under a different UID.

</details>

<details>
<summary>Which copy of a duplicate is kept?</summary>

With **Keep which copy** set to `first` (the default), the earliest occurrence in
the combined input wins and later copies are dropped. Set it to
`last_modified` to keep the copy with the newest `LAST-MODIFIED` timestamp
(falling back to `DTSTAMP` when that property is absent), which is what you want
when a second file is a more recently edited version of the first.

</details>

<details>
<summary>Are my events changed or reformatted?</summary>

No. Every surviving event is copied through **verbatim** — its properties, ordering,
folded continuation lines, `VALARM` sub-blocks and RFC 5545 text escapes are left
exactly as written. The only additions are a fresh `VCALENDAR` wrapper and, if you
provide one, an `X-WR-CALNAME` calendar-name line. Line endings are normalized to
CRLF so the output imports cleanly.

</details>

<details>
<summary>Is my calendar uploaded anywhere?</summary>

No. The merge runs entirely in your browser using WebAssembly — the `.ics` text you
paste is processed on your own device and is never sent to a server.

</details>
