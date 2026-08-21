# tasks-to-ical — competitor analysis (2026-08-21)

Scan run **before** implementing, per `create-next-tool` step 4. Everything below is a
paraphrase of publicly documented behaviour (READMEs, source, published feature lists);
**no competitor copy, branding, or trademark text was copied into the block, its manifest,
its descriptor, or its page.**

## Scope of the backlog row

> `tasks-to-ical` — "Converts dated tasks (todo.txt `due:`/`t:` tags) into an importable
> iCalendar (.ics) feed of VTODO/VEVENT entries." (type hint: pure)

## Dup check (why this is not an existing block)

`ls blocks/ | grep -iE 'ical|ics|task|todo'` surfaced nine neighbours. Each was read
(`core/src/lib.rs` + descriptor) before deciding:

| Existing block | What it does | Overlap |
| --- | --- | --- |
| `csv-to-ics` | CSV **with a header row** → VEVENT-only calendar. Requires `title` + `start` columns; the engine is a `csv::Reader` over aliased columns | Same output family, **completely different input** (spreadsheet columns, not todo.txt syntax) and it can only emit VEVENT — no VTODO, no priority, no completion status, no `+project`/`@context` |
| `ics-parse` | Reads an `.ics` **in**, reports its components | Opposite direction |
| `ics-to-csv` | `.ics` → CSV | Opposite direction |
| `ics-merge-dedupe` | Merges several `.ics` files | Operates on existing calendars |
| `ics-timezone-shifter` | Rewrites zones inside an existing `.ics` | Operates on existing calendars |
| `task-format-converter` | todo.txt ⇄ Markdown ⇄ JSON ⇄ CSV | Converts **between task formats**; iCalendar is not one of its targets and it emits no calendar semantics |
| `todo-organizer` | Free-form brain-dump → prioritised Markdown checklist | No dates parsed, no calendar output |
| `recurring-task-expander` | Expands `rec:1w`-style repeats into dated instances | Produces **task text**, not iCalendar; complementary (its output can be piped into this tool) |
| `task-list-summarizer` | Markdown checkbox stats | Counting, not conversion |
| `calendar-freebusy-overlap` | Free/busy intersection | Scheduling maths |

No block turns todo.txt into iCalendar. **Not a duplicate — built.**

## Competitors reviewed

1. **topydo** (`topydo/topydo`, console todo.txt app) — its README advertises "additional
   output formats to iCalendar, JSON and Graphviz Dot". Reading its iCalendar printer, the
   documented VTODO mapping is: uid from an `ical:` tag on the task (random when absent),
   `summary` = task text, `description` = the full source line, `priority` numeric
   (A→1, B→5, C–F→6–9, unmapped→9, none→0), `dtstart` from the task's start (threshold)
   date, `due` from the due date, `created` from the creation date, `completed` from the
   completion date combined with midnight. It does **not** emit `STATUS`,
   `PERCENT-COMPLETE`, `CATEGORIES`, or recurrence.
2. **"TODO.TXT to VTODO ICS" script** (public gist, shunf4) — parses `x` completion, the
   two bare `YYYY-MM-DD` dates, `(A)`–`(Z)` priority, `due:`, `+project`, `@context`, and
   bare URLs; strips metadata out of `SUMMARY` and keeps the raw line in `DESCRIPTION`.
   Emits `SUMMARY`, `DESCRIPTION`, `UID`, `CATEGORIES` (projects), `URL`, `CREATED`,
   `DTSTART`, `COMPLETED`, `DTSTAMP`, `LAST-MODIFIED`, `DUE`, `STATUS:COMPLETED`, numeric
   `PRIORITY`. Documented gaps: no `rec:` handling, no percent-complete, no alarms.
   It parses `t:` and `h:` but maps neither.
3. **OneCal ICS File Generator** (web) — form-driven `.ics` builder for Google/Outlook/
   Apple. Table stakes it advertises: runs entirely in the browser with nothing uploaded,
   recurring events, a meeting URL field, and a reminder that is written as a `VALARM`
   block with a user-chosen lead time.
4. **AnyOnlineTool ICS Editor** (web) — notable because it explicitly supports the
   `VTODO` component alongside `VEVENT`/`VJOURNAL`/`VTIMEZONE`, and lets you paste text
   rather than upload a file.
5. **giga.tools iCal Event File Creator / CalendarBridge ICS generator** (web) — the
   generic "paste details, download .ics" tier. Shared table stakes: free, no signup, a
   downloadable file rather than copy-only output, an arbitrary number of reminders, and
   explicit "imports into Google Calendar, Outlook and Apple Calendar" support claims.

(A sixth candidate, a blog walkthrough on generating a calendar from todo.txt, now 404s;
it was replaced by competitor 5 rather than running the scan with fewer sources.)

## Table stakes → decision

| Capability | Seen in | Verdict | Where it landed |
| --- | --- | --- | --- |
| Parse `x` completion marker | 1,2 | **in-model** | `STATUS:COMPLETED` + `PERCENT-COMPLETE:100` + `COMPLETED:` |
| Parse completion + creation dates | 1,2 | **in-model** | `COMPLETED`, `CREATED` |
| Parse `(A)`–`(Z)` priority | 1,2 | **in-model** | numeric `PRIORITY` (A→1, B→5, C–Z→9) |
| Parse `due:` | 1,2 | **in-model** | `DUE` (VTODO) / `DTSTART` fallback (VEVENT) |
| Parse `t:` threshold as the start date | 1 (`dtstart`) | **in-model** | `DTSTART` |
| Task text cleaned of metadata into `SUMMARY` | 1,2 | **in-model** | known tags + priority + dates stripped |
| Raw line preserved in `DESCRIPTION` | 1,2 | **in-model** | `DESCRIPTION` = original line |
| `+project` / `@context` → `CATEGORIES` | 2 | **in-model** | `CATEGORIES` (both, deduped, in order) |
| Stable/overridable UID | 1 (`ical:` tag) | **in-model** | `uid:`/`id:` tag, else a deterministic slug |
| `STATUS` on open tasks | 2 (partial) | **in-model** | `STATUS:NEEDS-ACTION` |
| `PERCENT-COMPLETE` | gap in 1 **and** 2 | **in-model** | emitted for completed tasks |
| VTODO **and** VEVENT output | 4 (VTODO), 3,5 (VEVENT) | **in-model** | `component` param — the headline gap: 1 and 2 emit VTODO only, 3 and 5 emit VEVENT only |
| Configurable reminder lead time | 3,5 | **in-model** | `reminder_minutes` (0 = off), slider on the page |
| Reminder anchored correctly per component | — | **in-model** | `TRIGGER;RELATED=END` for VTODO (RFC 5545 anchors that to `DUE`), plain `TRIGGER` for VEVENT |
| Name the calendar | 3,5 | **in-model** | `calendar_name` → `X-WR-CALNAME` |
| Download the result as a file | 3,5 | **in-model** | page `format = "text"` gets a Download link for free |
| Runs locally, nothing uploaded | 3,5 | **in-model** | WebAssembly in the page; also CLI + chat |
| Timed deadlines, not just all-day | 3,5 | **in-model** | `due:2026-08-25T14:00` → timed, `duration_minutes` for the VEVENT span |
| Floating vs UTC anchoring | 5 (implied by import claims) | **in-model** | `timezone` param, matching `csv-to-ics` |
| Filter which tasks are exported | — (own addition; 1 has task filtering upstream) | **in-model** | `include` (dated / all) + `skip_completed` |
| Deterministic bytes (clean re-imports) | — (own addition) | **in-model** | fixed `DTSTAMP`, derived UIDs |
| RFC 5545 CRLF + 75-octet folding + TEXT escaping | correctness baseline | **in-model** | shared emission helpers |
| Recurrence expansion (`rec:`) | gap in 1 **and** 2 | **out-of-model here** | `rec:` is recognised and stripped from `SUMMARY`; expanding it duplicates `blocks/recurring-task-expander`, which is the right upstream step. Not silently dropped — the descriptor and the page both say so. |
| `RRULE` passthrough | 3 ("recurring events") | **out-of-model** | todo.txt `rec:` is not an RRULE (it has no COUNT/UNTIL/BYDAY vocabulary); a lossy auto-translation would produce wrong repeats. Listed, not built. |
| Bare-URL detection → `URL` property | 2 | **out-of-model** | left in `SUMMARY` where the user wrote it; guessing which of several URLs is *the* task URL is not a deterministic rule |
| Meeting/conferencing URL field | 3 | **out-of-model** | there is no todo.txt tag for it, and inventing one would not round-trip with any todo.txt client |
| Two-way sync / write back to a task app | 3,5 | **out-of-model** | needs network + accounts; gizza blocks are local and pure |
| Uploading an existing `.ics` to edit | 4 | **out-of-model** | that is `blocks/ics-parse` + `blocks/ics-merge-dedupe`, already built |

## UX control patterns adopted

- **Preset chips** (`[[example]]`) — competitors 3 and 5 lead with prefilled example forms;
  four chips cover the four real shapes: a dated backlog, a VEVENT deadline calendar, a
  completed-work archive, and timed deadlines with a reminder.
- **Slider** for `reminder_minutes` and `duration_minutes` — 3 and 5 both use a lead-time
  picker rather than a free-text box.
- **Friendly enum labels** (`[input.labels]`) — "VTODO" and "floating" are jargon; the
  select shows what each choice means for the calendar app the user actually has.
- **Multiline textarea** for `tasks` — todo.txt is inherently one-task-per-line.
