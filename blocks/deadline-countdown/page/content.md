## About this tool

The **Deadline Countdown** turns a plain-text task list into an urgency-sorted schedule. Put one task on each line and include a due date such as `2026-08-01`, `2026-08-01 17:30`, an ISO datetime, or a `due:` / `deadline:` marker. The tool calculates each task's time remaining from your reference `now`, labels the status, and sorts overdue work before upcoming deadlines.

It is useful for daily triage, release checklists, renewal reminders, and turning a quick note into a deadline report. The reference time is explicit so results are reproducible in docs, tests, and CLI runs.

## What you can control

- **Reference date/time** — set the exact moment to count from, such as `2026-07-31 12:00`.
- **Output format** — choose aligned text, Markdown, JSON, or CSV.
- **Include completed tasks** — by default, lines starting with `x`, `[x]`, `done:`, or `✓` are skipped.
- **Due-soon window** — choose how many days ahead should be labeled `DUE SOON`.

## Worked example

Tasks:

```text
Submit taxes due: 2026-07-30
Ship launch due: 2026-07-31 16:00
Renew cert due: 2026-08-05
```

With `now = 2026-07-31 12:00`, the output puts **Submit taxes** first as overdue, **Ship launch** next as due today in four hours, and **Renew cert** after that as due soon.

## Limits and edge cases

- Date parsing is deterministic and local. Use `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, `YYYY-MM-DDTHH:MM[:SS]`, RFC 3339, `YYYY/MM/DD`, `MM/DD/YYYY`, or `DD.MM.YYYY`.
- The tool does not connect to calendars, issue trackers, or reminder apps; paste exported tasks as text.
- Relative phrases like "tomorrow" or "next Friday" are not parsed. Convert them to explicit dates first.
- Output is capped at 1,000 dated tasks to keep browser and CLI runs responsive.

## FAQ

<details>
<summary>How does the sorting work?</summary>

Overdue tasks sort first, with the most overdue item at the top. Upcoming tasks then sort by nearest due date. The status label is derived from the same time difference: `OVERDUE`, `DUE TODAY`, `DUE SOON`, or `LATER`.

</details>

<details>
<summary>What counts as a completed task?</summary>

By default, lines beginning with `x `, `[x]`, `done:`, or `✓` are skipped. Turn on **Include completed tasks** if you want those lines counted and sorted too.

</details>

<details>
<summary>Can I use it with a todo.txt file?</summary>

Yes. Paste the relevant lines and include dates either inline or after `due:` / `deadline:`. For example, `Finish report due: 2026-08-02` and `Call vendor 2026-08-01` both parse.

</details>

<details>
<summary>Why is the reference time required?</summary>

A fixed `now` makes the result repeatable across browser, CLI, and tests. It also avoids timezone surprises from hidden system clocks; enter the wall-clock date/time you want the countdown to use.

</details>
