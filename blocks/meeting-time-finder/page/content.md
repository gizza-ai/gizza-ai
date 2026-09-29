## About this tool

Meeting Time Finder searches one calendar date across multiple IANA timezones and ranks candidate meeting starts by fairness. It checks each participant's local working hours, optional per-person hours, local weekends and daylight-saving transitions, then returns the best slots with a per-person breakdown.

Use `Name@Zone` for readable labels, or just the zone name when labels do not matter. Add `:start-end` to override one person's workday, for example `Mina@Asia/Tokyo:10-18` or `Ops@America/Chicago:22:00-06:00` for an overnight support shift. The default output is a human summary; switch to `timeline` for a 24-hour overlap grid, `table` for a compact text table, `json` or `csv` for automation, or `ics` to copy the top-ranked slot into a calendar.

### Worked example

Participants:

```text
Alice@Europe/London, Bob@America/New_York, Chen@Asia/Tokyo:10-18
```

Date: `2026-10-01`, duration: `60`, granularity: `30`, display zone: `Europe/London`.

The result ranks the best 60-minute slots on that date, shows each person's local time, and marks whether each participant is in hours, partially in hours, outside hours or on a local weekend. If no slot works for everyone, leave "Rank closest compromises" enabled to see the least-bad options and who is affected.

### Limits and edge cases

- Use IANA timezone names such as `Europe/London`, `America/New_York` and `Asia/Tokyo`; city nicknames are not guessed.
- Up to 12 participants are accepted. Keep larger scheduling polls in a spreadsheet or scheduling app.
- Daylight-saving changes on the searched date are reported in the output so you can double-check the proposed slot.
- Weekend filtering is evaluated in each participant's own local timezone.
- The tool is deterministic: it uses the date you provide, not the current clock.

## FAQ

<details>
<summary>How do I give one person different working hours?</summary>

Add the hours after that participant's timezone with a colon: `Bob@America/New_York:8:30-16:30`. Entries without a suffix use the default workday start and end fields.

</details>

<details>
<summary>Can it handle overnight shifts?</summary>

Yes. A window such as `22:00-06:00` wraps past midnight in that participant's local timezone, which is useful for support handoffs and follow-the-sun teams.

</details>

<details>
<summary>What happens when no time is inside everyone's workday?</summary>

With "Rank closest compromises" enabled, the tool still returns the best-scoring slots and identifies who is outside hours or only partially covered. Turn that option off if you want an explicit no-slot result instead.

</details>

<details>
<summary>Does the calendar export create events in my calendar automatically?</summary>

No. The `ics` output is a plain calendar event for the top-ranked slot. Copy or download it and import it into your calendar app; no account or calendar API is used.

</details>
