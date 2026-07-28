# natural-language-task — competitor analysis (2026-07-28)

Tool: **natural-language-task** — turn a plain-English task sentence into a todo.txt line, extracting priority, +projects, @contexts and a `due:` date from phrases such as "tomorrow" or "next Friday". Browser-local / deterministic / no account. All copy below is paraphrased — no competitor wording, branding, or trademarks copied.

## Competitors scanned (top real tools)

1. **todo.txt CLI and mobile apps** — the canonical plain-text task format and common parser expectations: priority at the start, optional creation date, project/context tags, and `key:value` metadata such as `due:YYYY-MM-DD`.
2. **Todoist natural-language task entry** — typed phrases like priority markers, project/context-like routing, and natural due dates are interpreted while keeping quick-entry friction low.
3. **Remember The Milk / task quick-add parsers** — natural date phrases, priorities, tags/lists, and batch entry patterns appear in task quick-add UIs.

## Table-stakes params / behaviours

| Capability | Decision |
| --- | --- |
| Plain-English task text | **in-model** — required `text`, supports one task per line |
| todo.txt output shape | **in-model** — emits optional `(A)` priority, optional creation date, title, tags, then `due:YYYY-MM-DD` |
| Priority cues | **in-model** — `urgent`/`asap`/`critical`/`important`/`p1`→`(A)`, low/someday/minor→`(C)`, `p1`-`p4`→`(A)`-`(D)`, explicit `(A)`-`(D)` preserved |
| Relative due dates | **in-model** — today, tonight, tomorrow, day after tomorrow, weekdays with this/next, next week/month, `in N days/weeks/months` |
| Absolute due dates | **in-model** — ISO dates, M/D, month-name dates with optional year |
| Reproducible relative-date anchor | **in-model** — `reference_date`, defaults to current date on runtime surfaces |
| Default project/context | **in-model** — `project` and `context` fields append a tag only when the line lacks one |
| Multi-line brain dump | **in-model** — one non-blank input line becomes one output line |
| Toggle detection | **in-model** — booleans for creation date, priority detection and due-date detection |
| Full NLP / recurring tasks / reminders | **out-of-model** — deterministic Rust parser only; no LLM and no background scheduler |
| Time-of-day alarms | **out-of-model** — todo.txt `due:` is date-only here; times stay in the text |
| Cross-account task creation | **out-of-model** — this toolkit returns text; it does not call task-service APIs |

## UX control patterns competitors ship

- Quick-entry boxes are text-first, with optional chips/examples to teach the syntax. The page therefore uses a large textarea, a date field for reproducible relative dates, checkboxes for parser toggles, and example chips for the most common task shapes.
- Competitors expose tags/projects as separate fields or inline tokens. This tool supports both inline `+project`/`@context` and optional default fields.

## Design decisions

- Deterministic parsing, not ML: predictable output and private browser execution beat broad but surprising natural-language interpretation.
- Date phrases are stripped only when recognised; otherwise text is preserved.
- The earliest recognised date/priority cue wins per line, so output stays simple and todo.txt-compatible.
- Defaults are conservative: add creation date and detect priority/due date, matching todo.txt and quick-add expectations, while allowing toggles for literal text conversion.

Every table-stake above ends in the descriptor/page or the out-of-model list — none dropped silently.
