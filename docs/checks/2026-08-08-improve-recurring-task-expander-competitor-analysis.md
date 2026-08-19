# recurring-task-expander — competitor analysis (2026-08-08)

Scan run before finishing the build, per the create-next-tool loop. Notes below paraphrase observed behaviour and public documentation; no competitor copy, branding or trademarks are reused.

## Scope of the tool

Expand plain task-list recurrence tags into concrete dated task instances. Input is text such as `Pay rent due:2026-09-01 rec:+1m`; output is the next N dated lines or a structured export. This is not a calendar server, notification system, or full RFC5545 editor.

## Duplicate check

Existing nearby tools (`todo-organizer`, `task-list-summarizer`, `task-format-converter`, `task-format-converter`, `deadline-countdown`, `date-diff`, `parse-datetime`, `cron-next-runs`) either organize task text, convert task formats, calculate dates, or expand cron-style schedules. None parse todo.txt-style `rec:` tags and emit concrete task instances. Buildable as a distinct pure text/date utility.

## Competitors reviewed

### 1. todo.txt recurrence implementations (SwiftoDo / todo.txt tools)

- Use `rec:` tags on normal task lines.
- Support interval units such as days, weeks, months and years.
- Distinguish completion-based recurrence from strict due-date recurrence, commonly with a leading `+` (`rec:+1w`).
- Recreate a task with a new `due:` date when the old one is completed.
- Preserve the rest of the task line so projects, contexts and priority metadata survive.

### 2. Sleek recurring todos

- Uses the same todo.txt family syntax and creates a duplicate with an updated due date.
- The default recurrence advances from completion time; strict recurrence advances from the original due date.
- Month/year steps are treated as calendar steps rather than a fixed number of days.
- UI focuses on recurring todo maintenance, not batch preview/export.

### 3. Taskwarrior recurrence

- Keeps recurrence as first-class task metadata and materializes a limited number of future instances.
- Has a configurable instance limit to avoid infinite expansion.
- Supports both a recurrence rule and due/scheduled dates.
- Designed as a task database, so it can do status, dependencies and reports outside this tool's model.

## Table stakes → decisions

| Table stake | Decision |
|---|---|
| `rec:` tag syntax on task lines | In model — parse `rec:<value>` from each line. |
| `due:YYYY-MM-DD` anchor | In model — explicit anchor; blank falls back to the start date. |
| Units for day/week/month/year | In model — `d`, `w`, `m`, `y` plus long names. |
| Business-day interval | In model — `b`/business days, useful for work tasks. |
| Strict recurrence (`rec:+1m`) | In model — leading `+` keeps the original due-date grid and skips past occurrences. |
| Completion-based recurrence (`rec:1w`) | In model — overdue plain recurrences restart at the start date. |
| Weekday patterns | In model — `mon`, `mon,thu`, `weekdays`, `weekends`, `daily`. |
| Limit future instances | In model — `count` 1-100 per recurring task. |
| Preserve task metadata | In model — priorities, projects and contexts are left in the description; only `rec:`/`due:` are removed/replaced. |
| Month-end clamping | In model — Jan 31 + 1 month clamps to Feb 28/29. |
| Weekend shifting | In model — optional checkbox moves Saturday/Sunday interval results to Monday. |
| Multiple output forms | In model — text, Markdown, JSON and CSV. |
| Batch preview before committing | In model — this tool is exactly a preview/export surface rather than a task database mutation. |

## Considered but not built

- Full RFC5545 RRULE grammar — too broad for a lightweight pure task-list utility and already better served by calendar-specific tools.
- Natural-language schedules such as "every other Tuesday after payday" — ambiguous without locale and calendar context.
- Notification/reminder creation, task completion state and persistence — out of model for this repo; this block is deterministic text transformation.
- Time zones and times of day — output is date-only because todo.txt recurrence is date-oriented.
- Calendar file import/export — feasible later as a separate ICS tool, not needed for `rec:` expansion.

## Sources

- Search result/documentation snippets for SwiftoDo todo.txt recurrence.
- Search result/documentation snippets for bram85 todo.txt-tools recurrence wiki.
- Search result/documentation snippets for Sleek recurring todos.
- Search result/documentation snippets for Taskwarrior recurrence.
