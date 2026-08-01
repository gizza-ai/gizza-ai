# deadline-countdown — competitor analysis (2026-07-31)

Tool function: compute remaining/overdue time for a list of dated tasks and sort by urgency. Pure local text/date processing.

## Competitors scanned (paraphrased; no copy/branding reused)

1. **Due Date Urgency Calculator (Ahmad Free Tools)** — asks for a due date and effort/time-blocking inputs, then gives urgency and suggested schedule. Table stakes: explicit due date, urgency labels, deterministic calculation, schedule-friendly output.
2. **Pincoxe personal task queue** — markets automatic urgency sorting for pinned tasks. Table stakes: sort tasks by what needs attention soonest, keep overdue items visible, make focus order obvious.
3. **Deadline Countdown browser extension** — keeps important due dates close at hand with countdown-style display. Table stakes: visible countdown, simple task/deadline entry, quick status scan.
4. **General due-date tracking/project tools (Motion/Asana-style articles)** — emphasize due dates, progress/completion state, prioritization by urgency, and task lists. Table stakes: completion filtering, due-soon categorization, exportable status table.

Sources: web search results for deadline countdown / due date urgency / task due-date tracking.

## Table-stakes params / behaviour → decision

| Capability | In/out of model | Where it landed |
|---|---|---|
| Multiple tasks, one per line | in-model | `tasks` textarea |
| Due date/datetime parsing | in-model | core parser supports inline dates and `due:`/`deadline:` markers |
| Explicit reference time | in-model | required `now` parameter for deterministic browser/CLI results |
| Overdue / today / soon / later labels | in-model | status column |
| Urgency sorting | in-model | overdue first, then nearest upcoming |
| Completion filtering | in-model | `include_completed` checkbox |
| Adjustable due-soon window | in-model | `soon_days` integer parameter |
| Table, Markdown, JSON, CSV export | in-model | `format` enum |
| Calendar/task-app integration | out-of-model | listed only; this repo is local pure WASM, no external accounts |
| Natural-language relative dates | out-of-model | listed as limit; no NLP parser/model in current gizza model |
| Notifications/alarms | out-of-model | browser/app scheduling is outside a stateless local tool |

## Defaults chosen

`format=table`, `include_completed=false`, `soon_days=7`. The `now` field is explicit rather than hidden so examples and CLI checks are reproducible.
