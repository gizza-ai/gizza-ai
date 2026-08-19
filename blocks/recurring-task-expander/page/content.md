## About this tool

`recurring-task-expander` turns lightweight recurrence tags into concrete dated task lines. Paste a todo-style list with tags such as `rec:1w`, `rec:+2m`, `rec:mon,thu` or `rec:weekdays`, choose a start date and count, and the tool emits the next real `due:YYYY-MM-DD` instances.

The syntax follows common todo.txt recurrence conventions. A plain recurrence such as `rec:1w` is completion-based: if the task is already overdue, the next series starts from the `start` date. A strict recurrence such as `rec:+1m` keeps the original due-date grid and skips past dates, which is what you usually want for rent, invoices and calendar-like obligations.

Lines without a `rec:` tag pass through unchanged unless you set `default_rec`. Priorities, projects and contexts stay in the task text; only `rec:` and the old `due:` tag are removed before the new due date is appended.

### Supported recurrence values

- `1d`, `3d`, `day`, `days` — calendar days.
- `1b`, `2b`, `businessdays` — business days, Monday through Friday.
- `1w`, `2w` — weeks.
- `1m`, `3m` — calendar months, clamped to the end of short months.
- `1y` — years, including leap-day clamping.
- `mon`, `mon,thu`, `weekdays`, `weekends`, `daily` — weekday patterns.
- Prefix interval values with `+` to keep the fixed due-date schedule, for example `rec:+1m`.

### Limits and edge cases

- Maximum 200 task lines per run.
- `count` is capped at 100 instances per recurring task.
- Dates are date-only UTC-style `YYYY-MM-DD`; times and time zones are intentionally out of scope.
- Month and year recurrences clamp to the target month's last day (`2026-01-31 rec:+1m` yields `2026-02-28`).
- Weekend shifting affects interval recurrences; explicit weekday patterns already choose exact weekdays and are not shifted.

## FAQ

<details>
<summary>What is the difference between `rec:1w` and `rec:+1w`?</summary>

`rec:1w` is completion-based. If the task is overdue, the next generated occurrence starts from the `start` date. `rec:+1w` is strict: it keeps the original `due:` date grid and skips occurrences before `start`. Use strict recurrence for bills and deadlines that should stay on a fixed calendar rhythm.

</details>

<details>
<summary>Can I generate Monday/Thursday tasks without writing dates myself?</summary>

Yes. Use a weekday pattern such as `rec:mon,thu`. From the chosen start date, the tool walks forward and emits each matching weekday. It also accepts aliases such as `weekdays`, `weekends` and `daily`.

</details>

<details>
<summary>Does this update my task manager automatically?</summary>

No. It is a deterministic expander and exporter. Copy the text or Markdown output back into your task list, or use JSON/CSV output in an automation. It does not store tasks, mark anything complete or send reminders.

</details>

<details>
<summary>What happens to projects, contexts and priorities?</summary>

They are preserved as ordinary text. For example `(A) Pay rent +home @bank due:2026-09-01 rec:+1m` becomes `(A) Pay rent +home @bank due:2026-09-01`, then the next due dates are appended for later instances.

</details>
