## About this tool

Paste a flashcard review log and this tool replays each card's history to compute the next review date. It is designed for quick schedule audits: no account, no deck database, and no hidden state. Each run is deterministic, so the same rows and options produce the same due dates.

Use one row per review with a card name, a date, and a grade:

```text
card, date, grade
capital-of-peru, 2026-08-01, good
capital-of-peru, 2026-08-02, good
capital-of-peru, 2026-08-08, good
kanji-water, 2026-08-01, again
kanji-water, 2026-08-02, hard
```

Rows may be comma, tab, semicolon, pipe, or whitespace separated. Grades can be words (`again`, `hard`, `good`, `easy`), four-button numbers (`1`–`4`), or SuperMemo quality scores (`0`–`5`). A line containing only a card name declares an unreviewed card so it appears as new and due.

## Worked example

With SM-2 defaults and `today = 2026-08-24`, this input:

```text
card, date, grade
capital-of-peru, 2026-08-01, good
capital-of-peru, 2026-08-02, good
capital-of-peru, 2026-08-08, good
kanji-water, 2026-08-01, again
kanji-water, 2026-08-02, good
new-card-never-seen
```

produces a schedule table with each card's next due date, interval, days until due, repetition count, lapse count, and status. Use `output = csv` for spreadsheets, `output = json` for scripts, `output = explain` to see every state transition, or `output = forecast` to project the next reviews assuming the selected forecast grade.

## Limits and edge cases

- The run is capped at 5,000 review rows and 2,000 distinct cards.
- `today` is explicit. When it is blank, the latest review date in the log is used rather than the system clock.
- FSRS mode exposes difficulty, stability, and retrievability, and accepts a custom 21-number weight vector. It does not train or optimise weights from your history.
- There is no interval fuzz or workload balancing. This keeps output reproducible, but a real flashcard app may spread cards differently.
- Dates must be calendar dates in `YYYY-MM-DD` form. `/` and `.` separators are accepted, but ambiguous regional dates are not.

## FAQ

<details>
<summary>Which algorithm should I choose?</summary>

Choose `sm2` when you want the classic repetition/ease/interval model used by many simple flashcard schedulers. Choose `fsrs` when you want difficulty, stability, retrievability, and desired-retention controls. FSRS output is useful for comparing how the same history would schedule under a memory-model approach.

</details>

<details>
<summary>Can this import a full flashcard collection?</summary>

It imports a review log, not card fronts, answers, tags, or media. Export the columns you need as text or CSV: card identifier, review date, and grade. Optional state fields such as `ease=`, `interval=`, `reps=`, `lapses=`, `difficulty=`, `stability=`, and `last=` can be added to seed existing scheduler state.

</details>

<details>
<summary>Why do my dates differ from a flashcard app?</summary>

Apps often add interval fuzz, daily load balancing, buried siblings, deck limits, timezone rules, and version-specific scheduler tweaks. This tool intentionally computes a deterministic per-card schedule from the pasted history and parameters. Use the output as an auditable calculation, not as a full deck-sync replacement.

</details>

<details>
<summary>Does the tool train FSRS weights?</summary>

No. Training FSRS weights requires optimisation over a large history and is out of scope for a synchronous browser-local calculator. Paste a 21-number vector if you already have one; otherwise the built-in defaults are used.

</details>
