## Stop negotiating with the whole list

A to-do list is useful until every item starts competing for attention. Focus Picker turns a pasted
list into one next task plus a short, auditable reason. It reads common lightweight annotations such
as `!p1`, `due:tomorrow`, `due:+3d` and `est:90m`, scores the rows locally, and prints the full
ranking only when you want it.

Everything runs in your browser. Your task list never leaves your machine.

## Input format

Paste one task per line:

```
Fix login redirect !p1 due:tomorrow est:90m
Write release notes !p2 due:+3d est:2h
Refactor settings page !p3 est:1d
```

You can also paste columns from a spreadsheet:

```
Improve onboarding | p1 | +7d | 2d
Fix billing copy | p2 | +2d | 1h
Rebuild import flow | p0 | +14d | 5d
```

Priority is `p0` through `p4`, where lower numbers are more important. Due dates accept ISO
`YYYY-MM-DD`, `today`, `tomorrow`, `yesterday`, `eod`, `eow`, weekday names, or relative offsets such
as `+3d` and `+2w`. Effort accepts minutes, hours, working days, or a bare hour count: `30m`, `2h`,
`1.5h`, `2d`, `4`.

## Worked example

With **today** pinned to `2026-08-21`, this input:

```
Fix login redirect !p1 due:tomorrow est:90m
Write release notes !p2 due:+3d est:2h
Refactor settings page !p3 est:1d
Book retro room !p4 due:friday est:15m
```

returns a focus recommendation headed by the single task to do next, followed by the facts that made
it win: priority, due date, effort, score and method. Keep **Show full ranking** on when you want to
see the trade-offs; turn it off for a terse answer you can paste into a daily note.

## Scoring methods

| Method | Best for | Formula shape |
| ------ | -------- | ------------- |
| Balanced | Daily planning when importance, urgency and effort all matter | Priority + urgency + effort-ease |
| Deadline | Time-sensitive lists where missing a date is costly | Mostly urgency, with priority as a tie-breaker |
| WSJF | Backlog triage where small valuable work should rise | Value and time criticality divided by effort |
| Quick wins | Clearing useful small tasks without ignoring priority | Priority + urgency + extra effort-ease |
| Eisenhower | Separating urgent/important from noise | Do first / Schedule / Delegate / Drop quadrants |

The formulas are deterministic and printed in the summary so the result is explainable rather than a
black box.

## Limits and edge cases

- Up to **500 tasks**.
- Ambiguous local dates such as `3/4/26` are rejected; use ISO dates or relative words.
- `2d` of effort means two 8-hour working days.
- Blank priority and effort fields use the visible defaults. A task with no due date gets neutral
  urgency instead of being ignored.
- When **Pin overdue tasks** is on, overdue work is ranked above non-overdue work before score
  tie-breaking. Turn it off for strict formula ordering.
- This is deterministic parsing, not AI inference: if a line does not include `due:` or a due-date
  column, no date is guessed from prose.

## FAQ

<details>
<summary>How should I write priorities?</summary>

Use `!p0` for an emergency, `!p1` for high priority, through `!p4` for low priority. Lower numbers
score higher. If a line has no priority, the **Default priority** control supplies one.

</details>

<details>
<summary>What if I do not know the exact due date?</summary>

Use relative dates such as `due:tomorrow`, `due:+3d`, `due:+2w` or a weekday name. If the task truly
has no due date, leave it blank; it still scores with a neutral urgency value.

</details>

<details>
<summary>Which scoring method should I choose?</summary>

Start with Balanced for daily planning. Use Deadline when calendar risk dominates, Quick wins when
you are trying to clear small useful work, WSJF for product-style backlogs, and Eisenhower when you
need an urgent-versus-important quadrant label.

</details>

<details>
<summary>Why did an overdue task beat a higher-priority task?</summary>

The default **Pin overdue tasks** option moves overdue rows above non-overdue rows before normal
score tie-breaking. That mirrors the common planning rule that already-late commitments need a quick
explicit decision. Turn the option off if you want pure score ordering.

</details>

<details>
<summary>Can I export the ranking?</summary>

Yes. Choose Markdown for a paste-ready table or JSON for machine-readable output. The text format is
optimized for the common case: a direct focus pick plus a short justification.

</details>
