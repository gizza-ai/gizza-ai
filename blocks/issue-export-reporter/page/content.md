## About this tool

Issue Export Reporter turns a pasted issue-tracker CSV export into the flow metrics
teams usually need for a sprint review, operations readout, or lightweight delivery
retro. It recognizes common Jira exports (`Issue key`, `Status`, `Created`,
`Resolved`, `Story Points`, repeated `Sprint` columns) and Linear exports (`ID`,
`Status`, `Created`, `Started`, `Completed`, `Estimate`, `Cycle Name`) without a
network connection or API token.

The output can be a combined summary or a focused status, lead/cycle-time,
velocity, burndown, or per-issue report. Median-first percentiles are configurable,
finished and cancelled statuses are explicit, and non-standard exports can be
mapped with `field=header` pairs when the auto-detected headers are not enough.

### Worked example

Paste this tiny Jira-style export:

```csv
Issue key,Summary,Status,Story Points,Created,Resolved,Sprint
GIZ-1,Login page,Done,3,05/Jan/24 9:00 AM,08/Jan/24 5:00 PM,Sprint 1
GIZ-2,Fix crash,Done,2,05/Jan/24 9:00 AM,06/Jan/24 9:00 AM,Sprint 1
GIZ-3,Docs,In Progress,1,06/Jan/24 9:00 AM,,Sprint 1
```

With **Report** set to `summary`, the report includes:

```text
Overview
issues: 3
completed: 2
open: 1

Lead time
all: n=2 p50=1.0 days p85=3.3 days p95=3.3 days

Velocity
Sprint 1: 2 issues, 5 points
```

Switch **Output format** to `json` for a structured object, `csv` for spreadsheet
rows, or `markdown` for pipe tables that can be pasted into notes.

### Useful controls

- **Report** chooses the combined summary or a single table such as velocity,
  burndown, or the slowest items.
- **Delimiter** can auto-detect comma, semicolon, tab, or pipe-separated exports.
- **Column mapping** handles renamed fields, for example
  `status=State, resolved=Done At, points=Estimate`.
- **Done and cancelled statuses** make completion and throughput rules explicit.
- **Group distributions by** splits lead/cycle-time percentiles by assignee, type,
  priority, sprint, or status.
- **Bucket size** controls velocity and burndown periods: sprint, week, month, or
  day. `auto` uses sprints when an export includes them, otherwise weeks.
- **Business days only** drops Saturdays and Sundays from elapsed-time math.
- **Percentiles** accepts up to six values from 1 to 99, such as `30,50,70,85,95`.

### Limits and edge cases

- Input is capped at 5 MB and 50,000 data rows so browser runs stay responsive.
- The tool only uses timestamps present in the CSV. A plain export does not carry
  full transition history, so time-in-status and pause/resume calendars are out of
  scope.
- Lead time is `created → resolved/completed`. Cycle time is `started →
  resolved/completed` when a start column exists; otherwise the report explains
  that cycle time is unavailable.
- Velocity excludes cancelled/rejected work and counts an issue as completed when
  it has a done status or a completion timestamp.
- Business-day mode removes weekends but does not model holidays or working-hour
  calendars.
- No tracker connection is made. The CSV stays in the page and no credentials are
  requested.

## FAQ

<details>
<summary>Can this connect to Jira, Linear, or a private tracker URL?</summary>

No. This is an offline CSV reporter. Export the issues from your tracker, paste the
CSV here, and the metrics are computed locally without network access, accounts, or
API tokens.

</details>

<details>
<summary>What is the difference between lead time and cycle time?</summary>

Lead time starts at the issue's created timestamp and ends at its resolved or
completed timestamp. Cycle time starts at a detected `Started` / `In Progress`
column when the export has one. If the CSV has no start timestamp, cycle-time rows
are omitted rather than guessed from status names.

</details>

<details>
<summary>How should I handle custom status names?</summary>

Use **Statuses that count as done** and **Statuses that count as cancelled**. Both
fields are comma-separated and matched case-insensitively, so values such as
`Released`, `Merged`, `Won't Do`, or `Duplicate` can match your team's workflow
without editing the CSV.

</details>

<details>
<summary>Why are my percentile values different from a dashboard?</summary>

This tool uses the nearest-rank method over the issues visible in the pasted CSV.
Some dashboards interpolate percentiles, exclude outliers, use working calendars,
or read transition history that is not present in an export. Check the input rows,
`business_days`, and the done/cancelled status lists when reconciling numbers.

</details>
