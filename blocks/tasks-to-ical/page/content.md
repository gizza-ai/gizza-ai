## About this tool

A todo.txt file is a plain-text task list: one task per line, with `due:` deadlines,
`t:` start dates, `(A)`–`(Z)` priorities, `+project` tags and `@context` tags. Calendars
and task apps do not read it — they read iCalendar. This tool is the step between them.
Paste your list and it hands back a standards-compliant `.ics` document you can save and
import into Apple Reminders, Nextcloud Tasks, Thunderbird, Google Calendar, Outlook or
anything else that speaks iCalendar.

The conversion runs as WebAssembly inside this page, so your task list never leaves the
browser tab. The output is deterministic too: `DTSTAMP` is a fixed constant and the entry
IDs are derived from the tasks themselves, so the same list always produces the same bytes.
That keeps re-imports and version-controlled calendar files clean — importing twice updates
the same entries instead of duplicating them.

### A worked example

Paste this, leave every option alone:

```text
(A) Pay the hosting invoice +admin @office due:2026-08-25
Draft the quarterly report +work t:2026-08-20 due:2026-08-28
Buy milk @errands
```

You get back:

```text
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//gizza-ai//tasks-to-ical//EN
CALSCALE:GREGORIAN
BEGIN:VTODO
UID:pay-the-hosting-invoice@1.local
DTSTAMP:19700101T000000Z
DUE;VALUE=DATE:20260825
SUMMARY:Pay the hosting invoice
DESCRIPTION:(A) Pay the hosting invoice +admin @office due:2026-08-25
CATEGORIES:admin,office
PRIORITY:1
STATUS:NEEDS-ACTION
END:VTODO
BEGIN:VTODO
UID:draft-the-quarterly-report@2.local
DTSTAMP:19700101T000000Z
DTSTART;VALUE=DATE:20260820
DUE;VALUE=DATE:20260828
SUMMARY:Draft the quarterly report
DESCRIPTION:Draft the quarterly report +work t:2026-08-20 due:2026-08-28
CATEGORIES:work
STATUS:NEEDS-ACTION
END:VTODO
END:VCALENDAR
```

Three things to notice. The `(A)` became `PRIORITY:1` and the metadata came out of the
`SUMMARY`, while the untouched original line was kept in `DESCRIPTION`, so nothing you wrote
is lost. The `t:` date became `DTSTART`, so the second task shows a start as well as a
deadline. And "Buy milk" is missing — it carries no date, and the default export is dated
tasks only. Switch **Which tasks to export** to *Everything* to bring undated tasks along as
unscheduled to-dos.

### What each line can say

- **`x`** at the very start marks the task done. In todo.txt the next bare date is the
  completion date and the one after it is the creation date — both are read.
- **`(A)`** through **`(Z)`** set priority, and are accepted before or after the completion
  date, or as a `pri:A` tag. They map onto the RFC 5545 numeric scale: `(A)` → `PRIORITY:1`,
  `(B)` → `5`, everything else → `9`. A task with no priority letter gets no `PRIORITY` line.
- **`due:2026-08-25`** is the deadline; **`t:2026-08-20`** is the start (threshold) date.
  Add a time for a timed entry: `due:2026-08-25T14:30`, seconds optional.
- **`+project`** and **`@context`** words become `CATEGORIES`, deduplicated and kept in the
  order you wrote them.
- **`uid:`** or **`id:`** pins the entry's ID, which is what you want if you plan to re-import
  an evolving list into the same calendar.
- **`rec:`** and **`h:`** are recognised and taken out of the summary. Anything else shaped
  like `key:value` is left exactly where you put it, so a `url:` or a custom tag survives.
- Blank lines and lines starting with `#` are skipped, so a commented, sectioned file works.

### To-dos or events

**To-do (VTODO)** is the default and the more faithful mapping: each task keeps its `DUE`,
its `STATUS` (`NEEDS-ACTION`, or `COMPLETED` with `PERCENT-COMPLETE:100`), its `COMPLETED`
and `CREATED` dates and its priority. Task apps — Apple Reminders, Nextcloud Tasks,
Thunderbird — import these as real tasks you can tick off.

**Event (VEVENT)** exists because several calendar apps, Google Calendar among them, ignore
`VTODO` entirely. An event lands on the grid where you will actually see it. A whole-day task
runs from its `t:` date through its `due:` date; a timed one starts at its time and runs for
the event length you pick. Because an event must sit somewhere on the calendar, an undated
task cannot become one — it is reported by line number rather than quietly dropped.

### Times, timezones and reminders

A date with no time (`due:2026-08-25`) is written as `VALUE=DATE`, which every client shows as
a whole day and no timezone touches. A date with a time is anchored by the **How the times are
anchored** choice. **Floating** writes `20260825T143000` with no zone marker, so 14:30 reads
as 14:30 on whatever clock the reader has — the right choice for a personal deadline list.
**UTC** treats what you typed as already being UTC and appends the `Z`, so each calendar
converts it into local time — the right choice for a list shared across countries. There is
no per-task timezone tag; convert the times before pasting if they are mixed.

The reminder slider attaches a `VALARM` with `ACTION:DISPLAY` and the task text as its
message, from 5 minutes up to a week ahead. Leave it at 0 for no reminder. On a to-do the
trigger is anchored to the `DUE` date with `RELATED=END`, which is what RFC 5545 specifies;
on an event it counts back from the start.

### Limits and edge cases

- Up to **2000 tasks** per run. Past that the conversion stops and tells you to split the
  list rather than writing a truncated file.
- The event length applies only to a **timed** `VEVENT`; it accepts 1 to 1440 minutes (one
  day). Whole-day events and every `VTODO` ignore it.
- The reminder lead time accepts 0 to 10080 minutes (one week).
- `rec:` repeat tags are **not** expanded into `RRULE`s. todo.txt's `rec:1w` has no vocabulary
  for counts, end dates or "every second Tuesday", so translating it would invent repeats you
  did not ask for. Expand the repeats into dated tasks first, then convert.
- A line whose date will not parse, whose `t:` falls after its `due:`, or which is nothing but
  metadata with no task text, is reported by its line number — the file is not written with a
  broken entry in it.
- Commas, semicolons and backslashes are escaped as RFC 5545 requires, and lines longer than
  75 octets are folded with a leading space on the continuation, so a long task imports intact
  instead of being cut short by a strict parser.
- Entry IDs come from a `uid:` / `id:` tag when you set one. Without it the ID is built from
  the task text and its position, which stays stable as long as the list does.

## FAQ

<details>
<summary>How do I get the .ics file into my calendar or task app?</summary>

Save the output with an `.ics` extension — the Download link next to the result does it for
you. In Google Calendar on the web, go to Settings → Import & export and pick the file;
Outlook uses File → Open & Export → Import/Export → Import an iCalendar (.ics) file; Apple
Calendar and Reminders import with File → Import; Thunderbird uses Events and Tasks → Import.
Import into a new, empty calendar the first time, so you can delete the whole thing in one
click if the mapping was not what you expected.

</details>

<details>
<summary>Should I export to-dos or events?</summary>

Export **to-dos** if the app you are importing into has a task list — Apple Reminders,
Nextcloud Tasks and Thunderbird all read `VTODO`, and it is the only option that keeps
completion status, percent-complete and priority. Export **events** if your calendar ignores
`VTODO`, which Google Calendar does: the deadlines then appear on the grid as all-day or
timed entries. If you are unsure, export both and import them into two separate calendars —
the conversion is free and nothing is uploaded either way.

</details>

<details>
<summary>Why did most of my tasks disappear?</summary>

The default keeps only tasks carrying a `due:` or `t:` tag, because a working todo.txt is
mostly someday items that would clutter a calendar. Switch **Which tasks to export** to
*Everything* and the undated ones come along as `VTODO` entries with no `DUE`, which task apps
file under unscheduled. If a task you expected is dated and still missing, check the tag
spelling: `due:2026-08-25` with no space after the colon, and an ISO date — `due: tomorrow`
or `due:25/08/2026` will be reported as unreadable rather than guessed at.

</details>

<details>
<summary>What happens to tasks I have already finished?</summary>

They export by default, which is how you archive finished work: a line beginning with `x`
becomes `STATUS:COMPLETED` with `PERCENT-COMPLETE:100` and a `COMPLETED` date taken from the
first bare date after the `x`. That is genuinely useful in a task app — it shows the work as
done rather than pretending it was never there. Turn on **Leave out tasks already marked done
with x** to export only what is still outstanding.

</details>

<details>
<summary>Will my rec: repeating tasks repeat in the calendar?</summary>

No. A `rec:` tag is recognised and removed from the summary, but it is not turned into an
`RRULE`, and the entry appears once on its `due:` date. todo.txt's recurrence syntax simply
does not carry enough information — there is no way to say how many times, until when, or on
which weekdays — so any automatic translation would produce repeats you did not write. Expand
the repeats into individual dated tasks first, then paste the expanded list here. The original
line, `rec:` tag and all, is kept in each entry's `DESCRIPTION`.

</details>

<details>
<summary>My multi-day event ends a day late. Is that a bug?</summary>

No — look at the dates in the calendar rather than in the `.ics` text. iCalendar all-day ends
are *exclusive*, so an event running the 20th through the 22nd has to be written with `DTEND`
on the 23rd. That day is added for you, which means you should write the last day the task
actually runs in `due:`, not the day after it.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The conversion is a WebAssembly module running in this page, so your task list stays in
the browser tab. Load the page, disconnect from the network, and it still works. The same
conversion is available offline in the command-line tool if you would rather script it over a
todo.txt file on disk.

</details>
