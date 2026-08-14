## About this tool

Worklog Summarizer turns a plain timestamped activity log into a time report. Paste lines such as `09:00 @acme writing`, `2024-01-15 10:30 +review code review`, or `[2024-01-15 17:00] done`; each entry runs until the next timestamp, so you do not need explicit start/stop pairs.

Use `@project`, `+tag`, or `#tag` tokens to group work. The default report shows totals by project, by tag, and by day with percentages and ASCII bars. Switch to entry mode for a line-by-line timesheet, or export TSV, CSV, or JSON for spreadsheets and scripts.

Example input:

```text
2024-01-15 09:00 @acme +dev writing the parser
2024-01-15 10:30 @acme +review code review
2024-01-15 12:00 lunch
2024-01-15 13:00 @beta +dev bugfix
2024-01-15 17:00 done
```

Expected summary: 8h tracked, with `@beta` at 4h, `@acme` at 3h, the untagged lunch entry at 1h, and `+dev` totaling 5h 30m. The final `done` line closes the day and is not counted as its own task.

## Limits and edge cases

- Maximum pasted log size is 5,000,000 bytes.
- Blank lines plus lines starting with `#` or `//` are ignored.
- Supported timestamps include `YYYY-MM-DD HH:MM`, `YYYY/MM/DD HH:MM`, ISO `YYYY-MM-DDTHH:MM`, bracketed timestamps, bare `HH:MM`, compact `0900`, and 12-hour `9:00am` / `5:30pm`.
- Undated lines inherit the current day. If the clock goes backwards, the parser treats that as crossing midnight.
- A final open entry contributes zero extra time unless you set an end time. This avoids guessing that a forgotten entry ran overnight.
- Date filters only apply to dated entries. Undated entries are excluded when `from` or `to` is set.
- Rounding is applied per entry before totals are computed, matching billing-increment workflows.

## FAQ

<details>
<summary>What log format should I paste?</summary>

Start each activity line with a timestamp, then write the task text. Tags are optional but useful: `09:00 @client +dev build parser`. A later timestamp ends the previous entry.

</details>

<details>
<summary>How does the tool know when an entry ends?</summary>

Each entry runs until the next timestamped line. A line such as `done`, `end`, `stop`, `off`, or `---` closes the previous entry without adding a new task.

</details>

<details>
<summary>What happens if my last entry is still running?</summary>

The report marks it as open and does not add guessed time. If you know the real cutoff, set “Close final open entry at” to a time such as `17:30`.

</details>

<details>
<summary>Can I export the report?</summary>

Yes. Choose `table` for tab-separated rows, `csv` for spreadsheet import, or `json` for scripts. The readable summary is best for quick inspection.

</details>
