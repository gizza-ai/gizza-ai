## About this tool

Kanban boards are useful because they make workflow state visible: what is waiting, what is in progress, what is blocked, what is being reviewed, and what is already done. This tool turns a plain task dump into that structure without requiring a project-management account or a drag-and-drop app.

Paste one task per line, or paste notes that already contain headings such as `## Backlog` and `## In Progress`. Each line becomes a card. The parser looks for practical status hints — `status:done`, `[Blocked]`, `wip`, `in progress`, `needs review`, `waiting on`, and checked Markdown boxes — then routes the card into the closest matching column. It also pulls out common card metadata:

- `@alice` for assignee
- `#infra` or `[backend]` for labels
- `!high`, `P1`, or `priority:critical` for priority
- `due:2026-09-01` for due dates
- `- [x]` for completed cards

Choose Markdown sections when you want an editable board file, a Markdown table when you want a side-by-side snapshot for a doc or issue, or JSON when you want to feed the board into another tool.

### Worked example

Input:

```text
Draft the launch email @alice due:2026-09-01
Fix the login redirect — blocked
Rewrite the parser (wip) !high
- [x] Ship the v1 changelog
Migrate the staging database #infra
```

With the default columns, the Markdown output is shaped like this:

```markdown
# Kanban Board

## To Do (2)

- [ ] Draft the launch email @alice due:2026-09-01
- [ ] Migrate the staging database #infra

## In Progress (2)

- [ ] Fix the login redirect
- [ ] Rewrite the parser !high

## Done (1)

- [x] Ship the v1 changelog
```

If you add a `Blocked` column, the login redirect card is routed there instead of falling back to `In Progress` or `To Do`.

### Tips for better boards

- Name your columns in the order you want them rendered: `Backlog, To Do, In Progress, Blocked, Review, Done`.
- Use `status:column-name` or `[Column Name]` on a line when you need exact routing.
- Keep one logical card per line. Multi-line descriptions are treated as separate cards so no text disappears.
- Set a WIP limit to flag overloaded columns. The limit is advisory: cards are never dropped.
- Use `sort_by=priority` or `sort_by=due` after the board is routed if you want the highest-risk or earliest-due cards first.

### Limits and edge cases

- The input is capped at 500 task lines and 12 columns.
- Column matching is fuzzy but deterministic. A column named `Doing` catches `wip` and `in progress`; a column named `Review` catches `needs review`.
- Ambiguous cards go to `default_column`, or to the first configured column when no default is set.
- The tool is a converter, not a persistent board. It does not store cards, sync files, or provide drag-and-drop editing.
- JSON output includes parsed card fields, per-column counts, WIP-limit state, and totals. It is meant for automation, not for preserving every byte of the original notes.

## FAQ

<details>
<summary>Can I use my own column names?</summary>

Yes. Put a comma-separated list in the columns field, such as `Backlog, Ready, Doing, Blocked, Review, Done`. The output uses those names and order. Routing still works with common aliases: `wip` maps to a doing-style column, checked boxes map to a done-style column, and `blocked` maps to a blocked-style column when one exists.

</details>

<details>
<summary>How do I force a task into a specific column?</summary>

Add an explicit status marker to the line: `status:Review`, `status=blocked`, or `[In Progress]`. Explicit markers win over keyword guesses and section headings, so they are the safest way to disambiguate a task.

</details>

<details>
<summary>What happens to completed Markdown checkboxes?</summary>

A checked task (`- [x]`) is routed to the closest done-like column and is emitted as checked in Markdown output. Unchecked boxes stay unchecked and are routed by the same status, heading and keyword rules as plain lines.

</details>

<details>
<summary>Does the WIP limit prevent cards from appearing?</summary>

No. WIP limits are warnings, not filters. When a column exceeds the limit, the Markdown heading notes that it is over the limit and JSON output sets `over_limit: true`, but every card remains in the board.

</details>

<details>
<summary>Can it preserve nested subtasks or long card descriptions?</summary>

Not as nested card bodies. The input model is one card per line, which is what makes messy notes and exported checklists predictable. Indented lines and subtasks are treated as additional cards rather than being silently attached to the previous line.

</details>
