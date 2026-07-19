## Find the free time two calendars share

Trying to schedule with someone whose calendar you can't see live? Ask them for an
iCalendar export, paste both files above, and this tool lists every window where **both**
of you are free — restricted to working hours, on the days you choose, at least as long
as the meeting you need. Everything runs locally in your browser: the calendars never
leave your machine.

It reads standard **.ics exports** from Google Calendar (Settings → Import & export →
Export), Outlook (Save calendar), Apple Calendar (File → Export), Fastmail, Proton and
any other RFC 5545 producer. Busy time is taken from `VEVENT`s — including recurring
events — and from `VFREEBUSY` busy periods, so an availability-only export works too.
Events marked cancelled or free (`STATUS:CANCELLED`, `TRANSP:TRANSPARENT`) don't block
time.

## Worked example

Calendar A has a standup on Monday 2026-07-20 from 09:00–10:00 UTC; calendar B has a
lunch sync from 12:00–13:00 UTC. Scanning that single day between 09:00 and 17:00 with a
30-minute minimum returns:

```
Common free time — 2026-07-20 to 2026-07-20 · Mon–Fri 09:00–17:00 · UTC · ≥30m
Calendar A: 1 busy interval · Calendar B: 1 busy interval

Mon 2026-07-20  10:00–12:00  (2h)
Mon 2026-07-20  13:00–17:00  (4h)

2 free slots · 6h total
```

Pick the **JSON** output for machine-readable slots with ISO 8601 timestamps, or the
**iCalendar (.ics)** output to get the shared availability back as an RFC 5545
`VFREEBUSY` calendar (one `FREEBUSY;FBTYPE=FREE` period per slot) that other tools can
import.

## What's supported, and the limits

- **Recurring events:** `RRULE` with `FREQ=DAILY/WEEKLY/MONTHLY/YEARLY`, `INTERVAL`,
  `COUNT`, `UNTIL`, weekly `BYDAY` lists (e.g. `MO,WE,FR`), monthly ordinal `BYDAY`
  (e.g. `3MO` = third Monday), a single `BYMONTHDAY`, and `EXDATE` exceptions. Rules
  outside this subset (e.g. `BYSETPOS`, hourly frequencies) block only their first
  occurrence, and the result says so in a note.
- **Timezones:** event `TZID`s are resolved against the full IANA database (DST-correct),
  and the most common Windows/Outlook zone names ("Eastern Standard Time", "W. Europe
  Standard Time", …) are mapped automatically. Unknown TZIDs fall back to your selected
  timezone, with a note in the result.
- **Moved instances:** a rescheduled occurrence of a recurring meeting
  (`RECURRENCE-ID`) counts as extra busy time while the original slot also stays
  blocked — deliberately conservative, so a suggested slot is never secretly busy.
- **Caps:** each calendar up to 1 MiB of text; the scan range is 1–60 days; the minimum
  slot length is 5–720 minutes; up to 20,000 busy intervals after recurrence expansion.
- All-day events block the whole day; multi-day events block every day they span.

## FAQ

<details>
<summary>How do I get the .ics text of a calendar?</summary>

In Google Calendar open Settings → Import & export → Export, unzip the download and open
the `.ics` file in any text editor. In Outlook use File → Save Calendar (choose the date
range and full details or free/busy). On macOS/iOS Calendar use File → Export → Export.
Then copy the file's text and paste it here. If someone shares a `.ifb` free/busy file,
that works too — `VFREEBUSY` periods are understood.

</details>

<details>
<summary>Are recurring meetings like a weekly standup handled?</summary>

Yes. Daily, weekly, monthly and yearly `RRULE`s are expanded inside the scan range,
including `INTERVAL` (every 2 weeks), `COUNT`/`UNTIL` bounds, weekly `BYDAY` day lists,
monthly patterns like "third Monday" (`BYDAY=3MO`) and cancelled single occurrences
(`EXDATE`). Exotic rules (`BYSETPOS`, `BYWEEKNO`, hourly repeats) are outside the
supported subset: such an event blocks only its first occurrence and the result appends
a note, so nothing is dropped silently.

</details>

<details>
<summary>What if the two calendars are in different timezones?</summary>

That's fine — each event carries its own timezone (`TZID` or UTC `Z` times), and every
event is converted to the timezone you select before computing overlap, with daylight
saving handled by the IANA database. The working-hours window and the result times are
expressed in your selected timezone. Common Windows zone names used by Outlook exports
are mapped to IANA zones automatically; an unknown TZID falls back to your selected
zone and the result tells you.

</details>

<details>
<summary>Why is a slot shown when one calendar looks busy (or the other way around)?</summary>

Check three things. First, events marked free (`TRANSP:TRANSPARENT`, how "Free" shows in
Outlook) and cancelled events deliberately don't block time. Second, make sure the
selected timezone is the one you expect — the slot list is expressed in it, not in the
calendar's zone. Third, a gap shorter than the minimum slot length is hidden; lower
"Minimum slot length" to see it. In the other direction, a moved instance of a recurring
meeting keeps its original slot blocked too (conservative on purpose).

</details>

<details>
<summary>Is my calendar data uploaded anywhere?</summary>

No. The page is a static site running a WebAssembly module; parsing and the overlap
computation happen entirely in your browser. Your `.ics` text is never sent to a server,
which is exactly why this tool asks you to paste the export instead of a calendar URL.

</details>
