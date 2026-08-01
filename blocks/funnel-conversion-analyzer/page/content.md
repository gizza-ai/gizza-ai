## About this tool

**Funnel Conversion Analyzer** turns a raw event log into a step-by-step
conversion funnel. Paste a CSV/table where each row is one user event — a
user-id column and an event-name column — and the tool counts the unique users
who reach each step, then reports conversion from the top step, conversion from
the previous step, and the users lost (drop-off) at every stage.

List your funnel steps in order (for example `view,signup,purchase`), or leave
the steps field empty to auto-derive them from the distinct events in the order
they first appear. Everything runs locally in your browser as WebAssembly — your
event data is never uploaded.

### Worked example

Input:

```
user,event
u1,view
u1,signup
u1,purchase
u2,view
u2,signup
u3,view
```

With steps `view,signup,purchase`, the default table output is:

```
Funnel: 3 step(s), 3 total user(s)
1. view: 3 users | 100% of top
2. signup: 2 users | 66.67% of top | 66.67% from prev | drop 1 (33.33%)
3. purchase: 1 users | 33.33% of top | 50% from prev | drop 1 (50%)
Overall conversion: 33.33%
```

All three users viewed, two signed up (33.33% drop), and one purchased (50%
drop from signup) — an overall conversion of 33.33%.

### Options

- **Funnel steps** — an ordered, comma-separated list of event names. Leave empty
  to auto-derive the steps from the distinct events in first-seen order.
- **User column / Event column** — a header name or a 1-based column index. They
  default to the first and second columns.
- **Timestamp column** — optional. When set, a strict funnel requires each step to
  happen *after* the previous one in time (numeric epochs or ISO-8601 strings).
- **Strict order** — on by default: a user reaches step N only after completing
  every earlier step. Turn it off to count every user who performed a step,
  independently of the others.
- **First row is a header** — on by default so columns can be referenced by name.
- **Delimiter** — comma, tab, semicolon, or pipe.
- **Output** — a readable table (default) or JSON for scripting.

### Limits

- Conversion counts *unique users* per step, not raw event counts.
- Without a timestamp column, a strict funnel checks only that each earlier step
  was performed at all — add a timestamp to enforce the actual chronological order.
- A funnel needs at least two steps: supply them explicitly, or paste data with two
  or more distinct events.
- Very large logs are better summarized in a data pipeline; this page is best for
  paste-sized samples and quick audits.

## FAQ

<details>
<summary>What is the difference between strict and independent (unordered) funnels?</summary>

With **Strict order** on (the default), a user only counts toward a step if they
completed every earlier step — the classic drop-off funnel. Turn it off and each
step counts every user who performed that event, regardless of the others, which
is useful when steps can happen in any order.

</details>

<details>
<summary>How are conversion and drop-off rates calculated?</summary>

Each step shows conversion **from the top** (users at this step ÷ users at the
first step) and **from the previous** step (users here ÷ users at the step
before). **Drop-off** is the users lost since the previous step, and the drop-off
rate is that loss ÷ the previous step's users. All percentages are rounded to two
decimals.

</details>

<details>
<summary>Do I have to list the funnel steps myself?</summary>

No. Leave the **Funnel steps** field empty and the tool auto-derives the steps
from the distinct events in the order they first appear in your data. List them
explicitly when you want a specific order or only a subset of events.

</details>

<details>
<summary>Does the timestamp column matter?</summary>

Only for strict funnels. When you provide a **Timestamp column**, reaching a step
requires it to occur after the previous step in time, so a user who purchased
before signing up would not count as a completed signup→purchase. Numeric epochs
and ISO-8601 strings both sort correctly.

</details>

<details>
<summary>Is my event data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. Your
event log never leaves your device.

</details>
