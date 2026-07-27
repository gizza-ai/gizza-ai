## About this tool

**Text to Reminders** turns a pasted brain-dump into an importable iCalendar
`.ics` file. Write one task per line and the tool emits one reminder/task
component per non-blank line, with a due date when the line contains a recognised
phrase such as `tomorrow`, `Friday`, `in 3 days`, `2026-03-05`, `March 5`, or
`at 3pm`.

Parsing is deterministic: there is no LLM, no account, no upload and no guessing
beyond the documented date/time rules. Relative phrases are anchored on the
**Reference date** field so examples, CLI runs and page deep links are repeatable.
Priority keywords such as `urgent`, `asap`, `important` and `critical` can map to
iCalendar `PRIORITY:1`; low-priority words such as `someday` map to `PRIORITY:9`.

### Worked example

Input:

```text
Call the dentist tomorrow at 3pm
Urgent: submit expense report Friday
Buy milk
```

With reference date `2026-03-02`, priority detection on, undated lines kept and a
30-minute alarm, the output starts like:

```text
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//gizza-ai//text-to-reminders//EN
CALSCALE:GREGORIAN
BEGIN:VTODO
UID:todo-1-20260302@text-to-reminders
DTSTAMP:20260302T000000Z
SUMMARY:Call the dentist
DUE:20260303T150000
BEGIN:VALARM
ACTION:DISPLAY
DESCRIPTION:Call the dentist
TRIGGER;RELATED=END:-PT30M
END:VALARM
END:VTODO
```

### Options

- **Reference date** anchors relative phrases (`today`, `tomorrow`, weekdays,
  `in 2 weeks`). Leave it on today's date for real use, or set it explicitly for
  reproducible CLI/page tests.
- **Detect priority keywords** maps `urgent`, `asap`, `important`, `critical` to
  high priority and `someday`, `whenever`, `eventually` to low priority. Turn it
  off to keep those words in the task title.
- **Keep undated lines** keeps lines with no recognised date as tasks without a
  due date. Turn it off to emit only dated tasks.
- **Alarm minutes before due** adds a display alarm before each dated task. `0`
  means no alarm.

### Limits & edge cases

- This is a deterministic parser, not an AI assistant. If a phrase is not in the
  documented rule set, it remains part of the title instead of being guessed.
- Times are emitted as floating local iCalendar date-times (`DUE:YYYYMMDDTHHMMSS`)
  with no timezone database. Your importing calendar app interprets them in its
  local/calendar timezone.
- Repeating phrases such as `every Monday`, durations and end times are listed as
  out-of-model in the analysis and are not expanded.
- Each non-blank line becomes one task. Multi-line task descriptions are not
  merged; paste them as one line if they should stay together.
- iCalendar text values are escaped and long content lines are folded at the RFC
  line length.

## FAQ

<details>
<summary>Does this use AI to interpret my notes?</summary>

No. The parser is a fixed Rust implementation that recognises a documented set of
date, time and priority phrases. That makes output reproducible and keeps the tool
fully local in the browser.

</details>

<details>
<summary>What date does "tomorrow" or "Friday" use?</summary>

Those phrases are measured from the **Reference date** field. A bare weekday means
the next occurrence after the reference date; for example, with reference date
`2026-03-02` (a Monday), `Friday` resolves to `2026-03-06` and `next Monday`
resolves to `2026-03-09`.

</details>

<details>
<summary>What happens to lines with no date?</summary>

When **Keep undated lines** is enabled, they are included as reminders/tasks with
no `DUE` property. When it is disabled, undated lines are skipped so the output
contains only reminders with a resolved due date or time.

</details>

<details>
<summary>Can I import the result into Apple Calendar, Google Calendar or Outlook?</summary>

Yes. The result is a normal iCalendar `.ics` document containing task/reminder
components. Calendar apps vary in how prominently they show imported tasks, but the
file is standards-shaped text and can be saved with an `.ics` extension.

</details>
