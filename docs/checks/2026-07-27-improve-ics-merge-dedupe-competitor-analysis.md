# ics-merge-dedupe — competitor analysis (2026-07-27)

Function: combine one or more iCalendar (`.ics`) files into a single `VCALENDAR`
and remove duplicate `VEVENT`s. All findings below are paraphrased from public
product pages — no competitor copy, branding, or trademarks are reproduced.

## Top 3 real competitors

1. **iCalConverter — "Merge ICS Files"** (icalconverter.com/merge-ics-files).
   Free browser tool. Deduplicates by UID and, when two events share a UID, keeps
   the copy with the most recent `LAST-MODIFIED` / `DTSTAMP`. Offers an *optional*
   secondary check that flags events with matching titles and times (different
   UIDs) for manual review. Consolidates duplicate `VTIMEZONE` definitions,
   preserves summaries/descriptions/locations/attendees/recurrence rules/alarms/
   custom properties, and can assign a category tag to all events from a given file.
   No sort or calendar-rename control documented.

2. **CalendarMap — "ICS Merge & Deduplication Tool"** (calendarmap.app/ics-tools).
   Free, no login, runs in-browser. Multiple-file upload / drag-and-drop, up to
   10 MB each. Fixed matching: duplicates are identified by **UID + DTSTART**. A
   deliberately "one-click" UX — no choice of match criteria, keep-which-copy,
   sort, or calendar name.

3. **Apify — "ICS Calendar Feed Merger / Deduplicator"**
   (apify.com/bene123/ics-calendar-feed-merger-deduplicator). Merges up to 50
   *public* iCal/ICS **feed URLs** into one recurrence-aware, filtered, deduplicated
   dataset. Output is JSON/CSV event rows (a data pipeline), not a merged `.ics`.
   Developer/automation oriented (paid Apify actor), not a paste-and-download page.

## Table-stakes parameters, defaults, examples, UX patterns

| Capability | Competitors | Our decision | Default |
|---|---|---|---|
| Merge N files into one calendar | all 3 | **in-model** — concatenate file contents in one `ics` field | — |
| Dedupe by UID | all 3 | **in-model** — `dedupe_by = smart` / `uid_start` | `smart` |
| Dedupe by start + title (cross-app, different UIDs) | iCalConverter (secondary check), we make it first-class | **in-model** — `dedupe_by = start_title`; `smart` also falls back to it when no UID | `smart` |
| Match by UID + start time (recurrence-safe) | CalendarMap (UID+DTSTART) | **in-model** — `dedupe_by = uid_start` | — |
| Keep newest edited copy (LAST-MODIFIED/DTSTAMP) | iCalConverter | **in-model** — `keep = last_modified` | `keep = first` |
| Sort events chronologically | none explicit | **in-model** — `sort` (adds UX polish competitors lack) | `true` |
| Name the merged calendar (X-WR-CALNAME) | none | **in-model** — `calendar_name` | blank |
| Consolidate duplicate VTIMEZONE by TZID | iCalConverter | **in-model** — automatic, no param | — |
| Preserve events verbatim (props/alarms/recurrence) | iCalConverter | **in-model** — events copied verbatim; `RRULE` not expanded | — |
| Pass through VTODO/VJOURNAL/VFREEBUSY | implied | **in-model** — passed through unchanged | — |

### UX patterns matched
- Multiple-input UX: competitors take multiple files; the page uses one large
  multiline field where the user pastes/concatenates files (the block model is a
  single text input), documented in the field label + placeholder.
- Preset chips: `[[example]]` chips for the three headline scenarios (shared UID,
  same event different apps, keep newest edit) — competitors surface these as
  marketing bullets; we make them one-click.
- Enum `<select>`s with friendly labels for `dedupe_by` and `keep`; a checkbox for
  `sort`; a plain field for the optional calendar name.

## Out-of-model / deliberately excluded

- **File-feed URLs (Apify)** — fetching 50 remote iCal feeds is a network/automation
  pipeline, out of scope for a pure paste-in tool; the CLI can fetch a single URL,
  but multi-feed orchestration is not a block surface.
- **JSON/CSV event dataset output (Apify)** — that is a different tool
  (`ics-to-csv` already covers `.ics → CSV`); this tool's contract is `.ics → .ics`.
- **Per-file category tagging (iCalConverter)** — requires per-file boundaries the
  single concatenated `ics` field doesn't preserve; not exposed. Noted, not built.
- **Manual-review UI for fuzzy matches (iCalConverter)** — an interactive review
  queue is out of the stateless compute model; instead `start_title` performs the
  merge automatically and the docs explain exactly what it collapses.
- **RRULE expansion** — recurrence instances are not expanded; documented as a
  limit. Real expansion needs a timezone/DST database that isn't shipped.
