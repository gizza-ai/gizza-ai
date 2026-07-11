## About this tool

The **Timesheet Calculator** turns a plain-text work log into a costed timesheet. Write one entry
per line as a start–stop time range, tag it with a project, and the tool totals the hours for each
project and multiplies them by your hourly rate to get the billable amount. It runs entirely in your
browser — your log never leaves your device.

Each line follows a simple, forgiving format:

```
[YYYY-MM-DD]  START-END  PROJECT  [notes]
```

- **Times** can be 24-hour (`9:00`, `17:15`) or 12-hour with am/pm (`9am`, `5:30pm`).
- **Overnight shifts** are handled automatically: if the end is earlier than the start, the entry
  rolls past midnight — `22:00-02:00` is 4 hours.
- The **project/tag** is the word right after the time range; a leading `#` is stripped, so `#Acme`
  and `Acme` are the same project. Anything after the project is kept as notes.
- Lines that are blank or start with `#` or `//` are treated as comments and skipped.
- An optional **date** at the start of the line is recorded but does not affect the totals.

### Worked example

Log (default rate `100`, currency `$`, no rounding):

```
9:00-12:30 Acme kickoff call
13:00-17:15 Acme build feature
2024-01-15 10:00-11:00 #Beta review
```

Result (abridged JSON):

```json
{
  "projects": [
    { "project": "Acme", "minutes": 465, "hours": 7.75, "rate": 100.0, "amount": 775.0 },
    { "project": "Beta", "minutes": 60,  "hours": 1.0,  "rate": 100.0, "amount": 100.0 }
  ],
  "total_minutes": 525,
  "total_hours": 8.75,
  "total_amount": 875.0,
  "summary": "3 entries across 2 project(s) · 8.75 h · $875.00"
}
```

Acme's two blocks (3 h 30 m + 4 h 15 m) total 7 h 45 m = 7.75 h → $775; Beta's single hour → $100;
the grand total is 8.75 h and $875.

### Rates and rounding

Set a single **default hourly rate**, or give per-project overrides in the *Per-project rate
overrides* field as `Project=amount` pairs (`Acme=150, Beta=90`) — any project not listed falls back
to the default. Choose a **rounding increment** to bill in whole blocks: `6 minutes` is the
tenth-of-an-hour standard used in legal billing, and `15`/`30`/`60` minutes are common in payroll.
Each entry's duration is rounded to the nearest multiple before it is costed.

## FAQ

<details>
<summary>What time formats does the work log accept?</summary>

Times can be written 24-hour (`9:00`, `13:45`, `17:15`) or 12-hour with a meridiem (`9am`, `1pm`,
`5:30pm`). A bare hour like `9am` is treated as `9:00`. Each entry is a `START-END` range with no
space inside the range token, for example `9:00-12:30` or `8am-4:30pm`.

</details>

<details>
<summary>How are overnight or past-midnight shifts handled?</summary>

If the end time is earlier than the start time, the tool assumes the entry crosses midnight and adds
24 hours. So `22:00-02:00` and `10pm-2am` both come out to 4 hours. This means a single entry can be
at most 24 hours long.

</details>

<details>
<summary>What is the 6-minute rounding option?</summary>

Six minutes is one tenth of an hour (0.1 h), the standard billing increment used by many law firms
and agencies. With `6 minutes` selected, a 4-minute entry rounds up to 6 minutes (0.1 h) and a
1 h 5 m entry rounds to 1 h 6 m (1.1 h). Rounding is applied to each entry's duration (to the
**nearest** multiple, ties rounding up) before the amount is calculated. Pick `No rounding` to bill
the exact minutes.

</details>

<details>
<summary>Can different projects have different rates?</summary>

Yes. Put a default hourly rate in the *Default hourly rate* field, then add overrides in the
*Per-project rate overrides* field as `Project=amount` pairs separated by commas or new lines — for
example `Acme=150, Beta=90`. Any project you do not list uses the default rate. Leave the rate at 0
to total hours only, with no money.

</details>

<details>
<summary>Is my timesheet uploaded anywhere?</summary>

No. All parsing and arithmetic run locally in your browser via WebAssembly. Nothing you paste is sent
to a server, so it is safe for confidential client and payroll data.

</details>

<details>
<summary>What are the limits and edge cases?</summary>

Minutes must be `00`–`59` and 24-hour hours `0`–`23` (12-hour hours `1`–`12`). A line with no
recognizable `START-END` range, or an out-of-range time, is reported as an error naming the line
number. A line with no project token is grouped under `(no project)`. The date prefix is optional and
only recorded — totals are computed purely from the time ranges, so entries on different dates with
the same project are still summed together.

</details>
