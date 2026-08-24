## About this tool

Calendar exports are easy to get almost right and still be unusable. A meeting written as `DTSTART:20240710T090000` means "9 AM somewhere" until the importer decides what timezone "somewhere" is. A meeting written as `DTSTART;TZID=America/New_York:20240710T090000` has a zone, but it may need to become Berlin time for another system. This tool rewrites the timed parts of an iCalendar file so the result imports with the timezone model you choose.

A default conversion preserves the instant in time and expresses it in the target timezone. For example:

```
BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:demo@example
DTSTART:20240310T140000Z
DTEND:20240310T150000Z
SUMMARY:Standup
END:VEVENT
END:VCALENDAR
```

with target `Europe/Berlin` becomes a calendar with a generated `VTIMEZONE` and event times like:

```
DTSTART;TZID=Europe/Berlin:20240310T150000
DTEND;TZID=Europe/Berlin:20240310T160000
```

Use **Source timezone for floating times** when the input values have no `Z` and no `TZID`. Existing `TZID` parameters in the file are trusted in convert mode. Use **Relabel** mode when the wall-clock digits are already correct but the zone marker is wrong: `09:00Z` can become `09:00 America/New_York` rather than the equivalent UTC instant.

**Write output times as** controls the format of the result. `TZID` writes target-zone local values and can include a fresh `VTIMEZONE`. `UTC` writes `...Z` values and omits timezone definitions. `Floating` writes local wall-clock values with no zone marker, for importers that assign the zone outside the file.

Limits and edge cases:

- Up to 5,000 `VEVENT` blocks per run.
- Timed `DTSTART`, `DTEND`, `DUE`, `RECURRENCE-ID`, `EXDATE`, `RDATE`, and `RRULE` `UNTIL` values are rewritten.
- All-day `VALUE=DATE` events never move.
- `DTSTAMP`, `CREATED`, `LAST-MODIFIED`, alarms, attendees, descriptions, summaries, and unknown properties pass through.
- Existing `VTIMEZONE` blocks are removed before an optional fresh target-zone block is inserted.
- Ambiguous fall-back times use the earlier occurrence; spring-forward gap times roll forward one hour.
- Lines are unfolded before processing and folded back to 75-octet iCalendar lines.

## FAQ

<details>
<summary>What is the difference between convert and relabel?</summary>

**Convert** preserves the real instant. If a meeting is `20240310T140000Z`, Berlin output is `20240310T150000` because 14:00 UTC is 15:00 in Berlin on that date. **Relabel** preserves the written clock digits and changes what timezone they mean. Use relabel only when an export says the wrong timezone but the local meeting time is already what you want.

</details>

<details>
<summary>Will all-day events shift to the previous or next day?</summary>

No. All-day iCalendar values are written as `VALUE=DATE`, not as instants. The tool leaves those values unchanged, so a holiday or vacation day does not slide across a date boundary just because the target timezone is far away.

</details>

<details>
<summary>Why does the tool replace VTIMEZONE blocks?</summary>

Once every timed value is rewritten into the target zone, the old timezone definitions are stale. The tool removes them and, when writing `TZID` output, inserts one target-zone `VTIMEZONE` built from the IANA timezone data used by `chrono-tz`. If your importer already has its own timezone database, turn off the generated block.

</details>

<details>
<summary>Does this modify recurrence rules?</summary>

It keeps recurrence rules intact and only rewrites an `UNTIL` date-time when one is present. Date-only `UNTIL` values are left alone. Exception date lists (`EXDATE`) and recurrence date lists (`RDATE`) are shifted along with event starts and ends.

</details>

<details>
<summary>Can I paste only a VEVENT instead of a full VCALENDAR?</summary>

Yes. Bare event blocks are accepted and wrapped in a minimal `VCALENDAR` so the output imports cleanly. A full `.ics` file is preferred when you want calendar-level fields such as `VERSION`, `PRODID`, or `X-WR-TIMEZONE` preserved.

</details>
