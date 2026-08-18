## About this tool

Cohort analysis answers a different question from a single churn percentage: users who joined in
January may behave differently from users who joined in March. This tool starts from raw activity
rows, assigns each user to a signup cohort, and builds a cohort-by-period grid so you can compare
retention or churn at the same age across cohorts.

Paste an activity table with a user id and activity date. If you also paste a signup/users table,
cohorts come from the signup date; otherwise each user's first activity becomes their cohort date.
Choose monthly, weekly, or daily buckets, how many follow-up periods to display, whether cells show
percentages, counts, or both, and whether to output a readable table, CSV, or JSON. Cells that a
cohort has not aged into yet are shown as `-` rather than a misleading zero.

### Worked example

Activity events:

```csv
user,date
u1,2024-01-05
u1,2024-02-03
u1,2024-03-01
u2,2024-01-20
u2,2024-03-02
u3,2024-02-10
u3,2024-03-11
```

With **Cohort period** set to monthly, **Follow-up periods** set to `3`, and **Cell values** set to
counts and percent, the January cohort starts at P0 with two users, P1 shows the users active one
month later, and the weighted average row combines every cohort old enough to observe each period.
Switch **Metric** to churn to see period-over-period losses instead of retained users.

### Limits and edge cases

- Activity and signup inputs are capped at **50,000 rows** each.
- At most **1,000 cohorts** are rendered; use month/week granularity for long histories.
- **Follow-up periods** accepts **1–36**.
- Dates must be ISO-like (`YYYY-MM-DD`, an ISO timestamp, `YYYY-MM`, `YYYYMMDD`) or Unix epoch
  seconds/milliseconds. Ambiguous `03/04/2024` style dates are rejected instead of guessed.
- The tool treats a user as active in a period if they have at least one activity row in that period.
  Define "active" upstream before pasting the data.
- Activity before a user's signup date is ignored and reported in the notes.
- Users in the signup table with no activity are still included in the cohort size, so P0 may be less
  than 100%.
- This is user retention, not revenue retention. It does not compute MRR, NDR, GDR, LTV, SQL queries,
  saved dashboards, or benchmark overlays.

## FAQ

<details>
<summary>Do I need a separate signup table?</summary>

No. If **Signup/users CSV** is blank, each user's first activity date defines the cohort. Paste a
signup table when you want users with no activity to count in the denominator, or when signup and
first activity are different events.

</details>

<details>
<summary>What is P0?</summary>

P0 is the signup period itself: the month, week, or day containing the signup date. P1 is one full
period later, P2 is two periods later, and so on. Comparing P1 across cohorts is usually more useful
than comparing calendar months directly.

</details>

<details>
<summary>How is churn calculated?</summary>

Churn is period-over-period loss: users active in the previous observable period minus users active
in the current period, divided by the previous period's active users. If users come back after being
inactive, churn can be negative for that step.

</details>

<details>
<summary>Why are some cells a dash?</summary>

A dash means the cohort is not old enough to observe that period as of the analysis date. For
example, a March cohort cannot have P3 retention in April. Set **Analysis date** to reproduce a
specific reporting cut; otherwise the latest date in the input is used.

</details>

<details>
<summary>Is my user data uploaded?</summary>

No. The parser and retention calculations run in WebAssembly inside the browser. For the CLI, the
same deterministic Rust core runs locally. No account, warehouse connection, or remote service is
used.

</details>
