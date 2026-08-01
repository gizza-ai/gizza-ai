# text-to-reminders — competitor analysis (2026-07-27)

Tool function: extract dated tasks/reminders from free-form notes and emit them as
iCalendar `.ics` VTODO items. One WebSearch run; top real competitors skimmed below.
All notes are paraphrased — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **Novacal — Text to Calendar Event** (browser NL → `.ics`). Detects title, date, time,
   timezone and duration from a plain sentence such as an appointment "next Monday at 10 AM",
   then offers a ready-to-import `.ics` download. Runs entirely in-browser, no upload. Closest
   analogue to our model, but it targets a single VEVENT appointment per run, not a batch of
   tasks.
2. **Things — natural-language date input** (native app). Parses relative expressions the user
   types in their own words ("17 days from July 9", "tomorrow", weekday names) to set a task's
   when/deadline. Strong on relative-date phrasing; app-only, no `.ics` export surface.
3. **TaskNotes (Obsidian plugin)** — a built-in NL parser pulls structured fields out of one
   line ("Buy groceries tomorrow at 3pm @home #errands high priority"): title, date, time,
   `@context`, `#tags`, priority. Syncs to calendars via OAuth or an ICS feed. Rich field set;
   requires the Obsidian/plugin environment and accounts for sync.

## Table-stakes → decision (in-model / out-of-model)

| Capability | Seen in | Decision |
|---|---|---|
| Relative dates (today, tonight, tomorrow, day-after-tomorrow, weekday, next week/month, "in N days/weeks") | Things, TaskNotes, Novacal | **in-model** — anchored on a `reference_date` param |
| Absolute dates (ISO `YYYY-MM-DD`, `M/D[/Y]`, "March 5[, 2027]", "5 Mar") | Novacal | **in-model** |
| Time-of-day (at 5pm, 17:00, 5:30pm, noon, midnight, morning/afternoon/evening) | Novacal, TaskNotes | **in-model** — date-only ⇒ all-day `VALUE=DATE`, else floating `DATE-TIME` |
| Batch: many tasks at once | (our strength; others do one) | **in-model** — one VTODO per non-blank line |
| Priority from keywords (urgent/asap/important → high) | TaskNotes | **in-model** — `detect_priority` toggle → VTODO `PRIORITY` |
| Keep undated lines as tasks | task apps | **in-model** — `include_undated` toggle (VTODO with no `DUE`) |
| Reminder/alarm before due | "reminders" framing, task apps | **in-model** — `alarm_minutes` → `VALARM` `DISPLAY` trigger |
| `.ics` download, browser-local, no upload/account | Novacal | **in-model** — pure wasm, `format="text"` download link |
| Title extracted separately from the date phrase | Novacal, TaskNotes | **in-model** — matched date/time span stripped from `SUMMARY` |
| Timezone / `TZID` resolution | Novacal | **out-of-model** — no tz database shipped; floating local time (documented) |
| Duration / end-time → `DTEND`/`DURATION` | Novacal | **out-of-model** — VTODO tasks carry a due, not a span (listed, not built) |
| Recurrence (`RRULE`) from "every Monday" | task apps | **out-of-model** — listed, not built |
| `@context` / `#tags`, calendar OAuth sync, accounts | TaskNotes | **out-of-model** — needs backend/accounts; listed, not built |

## UX control patterns adopted
- `reference_date` as a native **date** picker, client-side default `today`.
- Booleans (`detect_priority`, `include_undated`) as checkboxes; `alarm_minutes` as a number.
- `[[example]]` preset chips (a dated brain-dump; a single appointment) — competitors ship
  presets/examples; chips are the declarative answer.
- Multiline textarea for the notes so pasted newlines survive.

## Descriptor shape decided
`text` (required, multiline), `reference_date` (date, default today), `detect_priority`
(bool, default true), `include_undated` (bool, default true), `alarm_minutes` (int, default 0).
Output: a single `VCALENDAR` with one `VTODO` per task.
