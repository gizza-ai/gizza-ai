# ics-agenda-view — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. Everything below is a
**paraphrase** of publicly documented behaviour — no competitor copy, branding, or trademarks are
reproduced, and no competitor asset is used. Out-of-model items are listed, not built.

## Competitors reviewed (top 3 reachable)

| # | Tool | Angle |
|---|------|-------|
| 1 | csvtoics.app — ICS viewer | Drop an `.ics` file, read every event grouped under date headings |
| 2 | ics-viewer.utils.com — ICS Calendar Viewer | Sortable table + month grid, filter/search, re-export |
| 3 | u2tool.com — iCal parser | Paste or upload, event table + JSON export for debugging |

A fourth candidate (icsviewer.com) renders entirely client-side and served only a "Loading…"
shell to a plain fetch, so it could not be assessed factually; the three above replace it.

## Table stakes observed

| Capability | 1 | 2 | 3 | Our decision |
|---|---|---|---|---|
| Paste raw `.ics` text (not just file upload) | – | – | yes | **In model** — `ics` multiline field is the only input; the page textarea accepts a pasted export |
| Events grouped under date headings, chronological | yes | – | – | **In model** — this is the tool's core output |
| All-day events listed first within a day | yes | – | – | **In model** — all-day lines are emitted before timed lines |
| Per-event: start/end, title, location | yes | yes | yes | **In model** — `details = compact\|normal\|full` |
| Per-event: description, organizer, status | yes | – | yes (JSON) | **In model** — `details = full` |
| "Repeats" / recurrence indicator | yes | yes | yes | **In model** — `(repeats)` marker on expanded occurrences |
| Cancelled-event badge | yes | – | – | **In model** — `include_cancelled` (default off) + `(cancelled)` marker |
| Multi-day event spans | yes | – | – | **In model** — occurrences are clipped per day and marked `(continued)` / `(continues)` |
| Timezone conversion for display | browser-local | browser-local | not expanded | **In model, better** — explicit IANA `timezone` param, DST-correct via chrono-tz; page pre-fills the browser zone |
| Date-range scoping ("all / upcoming / past") | – | yes | – | **In model** — `start_date` + `days` window (deterministic; no hidden clock) |
| Text filter / search across events | – | yes | – | **In model** — `filter` matches title, location and description, case-insensitively |
| Machine-readable export (JSON) | – | CSV/ICS | JSON | **In model** — `output = text \| markdown \| json` |
| Recurrence expansion into real occurrences | no (base date only) | no (base date only) | no (RRULE passed through) | **In model, differentiator** — `expand_recurring` expands DAILY/WEEKLY/MONTHLY/YEARLY with INTERVAL/COUNT/UNTIL/BYDAY/BYMONTHDAY + EXDATE |
| Free-gap detection between meetings | – | – | – | **In model, differentiator** — `show_gaps`, `day_start`/`day_end`, `min_gap_minutes` |
| Client-side only, nothing uploaded | yes | yes | yes | **In model** — wasm in the browser, matched |

## UX control patterns worth matching

- **Filter + live counts** (2): our page ships a `filter` text field and per-day/aggregate totals
  in every output format.
- **"When" scoping selector** (2): expressed as an explicit date window (`start_date` + a `days`
  slider) rather than a relative selector, so CLI and page results are reproducible.
- **Preset one-click views** (all three lead with a ready example): three `[[example]]` chips —
  a one-day agenda, a work-week with gaps, and the JSON export.
- **Native pickers**: `date` kind for `start_date`, `time` kind for `day_start`/`day_end`,
  `slider` for `days`, searchable timezone autocomplete with a `local-timezone` smart default.
- **Stated limits on the page**: 1 MiB input cap, 90-day window, 5000-occurrence cap, and the
  supported RRULE subset are documented in the FAQ instead of surfacing only as errors.

## Considered, out of model

- **Drag-and-drop file upload / `.zip` Google exports** — the page's field input takes pasted text;
  unzipping a calendar archive in the page shell is out of scope for this block.
- **Month/week calendar grid rendering** (1, 2) — a graphical grid is a page-shell rendering
  concern, not a block output; the agenda list is the text-first equivalent.
- **Re-export to `.ics`/CSV, or "add to Google Calendar" hand-off** (2, 3) — round-tripping and
  account hand-offs belong to `blocks/ics-to-csv`, `blocks/csv-to-ics` and `blocks/ics-merge-dedupe`;
  duplicating them here would be redundant.
- **Full RFC 5545 validation** (3 explicitly declines it too) — this tool reads calendars, it does
  not certify them.
- **Attendee availability across several calendars** — already shipped as
  `blocks/calendar-freebusy-overlap`.

## Relationship to existing gizza blocks (dup check)

- `blocks/ics-parse` — emits a flat JSON array of VEVENTs with no day grouping, no timezone
  conversion, no recurrence expansion and no gap analysis. Different deliverable.
- `blocks/calendar-freebusy-overlap` — needs **two** calendars and reports only the free windows
  common to both; it never lists the events themselves. Different deliverable.
- `blocks/ics-to-csv`, `blocks/ics-merge-dedupe`, `blocks/ics-timezone-shifter`, `blocks/csv-to-ics`
  are format conversions, not a reading view.

Not a duplicate: this block is the single-calendar, day-grouped **reading** view plus free-gap
detection, which no existing block provides.
