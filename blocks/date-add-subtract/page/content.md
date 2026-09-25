## About this tool

**Date Add & Subtract Calculator** shifts a date or datetime forward or backward by any combination of years, months, weeks, days, hours, minutes and seconds, and tells you exactly where you land. Use it for deadlines, notice periods, contract end dates, due dates, trial expiries, renewal dates, or "what date is 90 days from today".

Enter a start date, pick **Add** or **Subtract**, fill in the units you care about, and run. The result includes the shifted date and time, the weekday, whether it falls on a weekend, the ISO week, the day of the year, the total calendar days moved, the Unix timestamp, and a plain-English summary of the whole calculation.

### Start dates it accepts

The date field is forgiving. It reads `YYYY-MM-DD`, `YYYY-MM-DDThh:mm` and `YYYY-MM-DDThh:mm:ss`, `YYYY/MM/DD`, `MM/DD/YYYY`, `DD.MM.YYYY`, month-name forms such as `19 June 2026` or `June 19, 2026`, and the keywords `today`, `tomorrow` and `yesterday`. A date-only input is treated as midnight, and the result is printed without a clock unless a time component is involved.

### How the units are applied

Order matters, and this tool applies units the way calendars actually work:

1. **Years and months first.** Years fold into months, then the calendar month is shifted and the day-of-month is clamped to the end of the target month. Jan 31 plus one month is Feb 28 (Feb 29 in a leap year), not Mar 3.
2. **Weeks and days next.** Weeks are seven day-steps. In business-day mode those steps count working days only.
3. **Hours, minutes and seconds last**, as a plain duration on top of the resulting date.

All unit fields take whole numbers, positive or negative. Half units are rejected on purpose — use a smaller unit instead, for example `36` hours rather than `1.5` days. The direction comes from the **Operation** field, so a negative value inside a **Subtract** run moves forward again.

### Business-day mode

Turn on **Count working days only** to step over Saturdays and Sundays, and list dates in **Holidays to skip** to exclude those too. Listing holidays switches on business-day mode by itself, even with weekend skipping off. In this mode:

- Day and week steps advance only across working days, so `+10 days` means ten working days.
- If the year/month part of the shift lands on a weekend or holiday, the result rolls to the nearest working day in the direction of travel.
- The output reports `business_days_moved` (the working-day steps requested) alongside `calendar_days_moved` (the real distance on the calendar) and `skipped_days` (how many non-working days were stepped over).

Holidays accept the same date-only formats as the start date and can be separated by commas, semicolons or newlines — spaces are not separators, so `4 July 2026` stays a single entry. Duplicates are ignored.

### Worked example

Start `2026-12-18`, **Add**, `15` days, working days only, holidays `2026-12-25, 2026-12-26, 2027-01-01`. Fifteen working-day steps from a Friday skip four weekends and three listed holidays, so the calendar distance is far longer than fifteen days and the result is a Monday–Friday date in January. The summary line reads back the whole calculation in words, and `skipped_days` shows how many days were stepped over without being counted.

CLI example:

```bash
gizza tool date-add-subtract date=2026-12-18 operation=add days=15 skip_weekends=true holidays="2026-12-25, 2026-12-26, 2027-01-01"
```

### Limits and edge cases

Each unit has a generous cap (10,000 years, 120,000 months, 520,000 weeks, 3,650,000 days, and correspondingly large hour/minute/second values), and results outside the supported calendar range are reported as an error rather than silently wrapping. Business-day mode walks day by day, so it is capped at 200,000 working-day steps per run — turn weekend and holiday skipping off for spans larger than that. At most 1,000 holiday dates are accepted. All arithmetic is calendar arithmetic on the date you type: there is no timezone conversion and no daylight-saving adjustment, so a datetime in, out, and the hours added between them are all in the same implied zone.

## FAQ

<details>
<summary>What date is 30, 60 or 90 days from today?</summary>

Type `today` in the start field, leave **Operation** on **Add**, and put `30`, `60` or `90` in the **Days** field. The result gives the calendar date, its weekday and its ISO week. Use **Subtract** with the same numbers for 30, 60 or 90 days ago.

</details>

<details>
<summary>How does it handle adding a month to the 31st?</summary>

Months are calendar months, and the day-of-month is clamped to the last valid day of the target month. So `2026-01-31` plus one month is `2026-02-28`, and plus three months is `2026-04-30`. Clamping is not reversed on the way back: adding a month and then subtracting a month can land on a different day than you started on, which is normal calendar behaviour rather than a rounding bug.

</details>

<details>
<summary>How do I count business days instead of calendar days?</summary>

Turn on **Count working days only**. Day and week steps then advance across Monday–Friday only, so `+10 days` means ten working days, and a result landing on a weekend rolls to the next working day in the direction you are moving. The output shows both `business_days_moved` and the real `calendar_days_moved`.

</details>

<details>
<summary>Can I exclude public holidays too?</summary>

Yes. List them in **Holidays to skip**, separated by commas, semicolons or newlines — for example `2026-12-25, 2026-12-26, 2027-01-01`. Each entry accepts the same date-only formats as the start field. Adding any holiday enables business-day mode on its own, so you can skip holidays with or without weekend skipping.

</details>

<details>
<summary>Why does it reject 1.5 days or 2.5 months?</summary>

Every unit must be a whole number, because a fractional month or business day has no single correct meaning on a calendar. Express the remainder in a smaller unit instead: `36` hours rather than `1.5` days, or `2` months and `15` days rather than `2.5` months.

</details>

<details>
<summary>Does it handle leap years and times of day?</summary>

Yes. Leap days are real calendar days, so a span crossing 29 February counts it, and `2024-02-29` plus one year clamps to `2025-02-28`. If your start value includes a time, hours, minutes and seconds are applied after the date math and can roll the result onto the previous or next day; the output then reports both `result_date` and `result_time`.

</details>

<details>
<summary>Is my data sent anywhere?</summary>

No. The calculation runs as WebAssembly inside your browser tab. The dates and holiday lists you enter are never uploaded, so the tool works offline once the page has loaded.

</details>
