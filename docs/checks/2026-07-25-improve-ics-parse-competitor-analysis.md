# ics-parse — competitor analysis (2026-07-25)

Function: parse an iCalendar (.ics) document into a **structured JSON list of events**
(title, start/end times, location, recurrence, organizer/attendees, status, categories).
Distinct from the existing `ics-to-csv` block, which flattens VEVENTs into a CSV *table* and
does **not** parse RRULE, ORGANIZER, or ATTENDEE. The JSON surface + recurrence/people
parsing is the new capability here.

All notes below are paraphrased from public tool descriptions — no copy, branding, or
trademarks are reproduced.

## Competitors skimmed

1. **jsontoolhub.com — iCalendar to JSON Converter** — paste or drag-drop `.ics`; outputs a
   JSON object per event with `uid`, `start`/`end` (normalized to UTC), `summary`,
   `description`, `location`, `attendees` (array of name/email), and `recurrence` (RRULE
   parsed into a structured object: frequency, count, day patterns). Advertises "Parse RRULE
   into JSON structure", "Extract participant information", "Convert to UTC or local time",
   "Handle calendars with multiple events". UX: copy in/out, format toggle, reverse
   conversion (JSON→iCal). No explicit per-field toggles documented.
2. **icalconverter.com — ICS to JSON** — browser-based, no upload; "structured output with
   ISO 8601 dates" for REST-API / dev integration. Reverse + fix/merge/split ICS also offered
   (out of scope for a single parser tool).
3. **u2tool.com — iCal Parser** — paste or upload `.ics`; shows a readable **table** AND a
   **JSON export** of event objects. Fields: summary, start, end, location, description,
   organizer, attendees, status, categories, RRULE; UID + timezone IDs are processed. Framed
   as an inspection/QA/handoff utility, browser-only, never writes back to a calendar.
4. **ics-to-json (npm)** — reference JS library: event objects with summary/start/end/
   location/description/uid and RRULE; dates as the raw/normalized value. (Page 403'd to the
   fetcher; shape confirmed from the ecosystem docs + the other tools above.)

## Table-stakes → where each lands

| Table-stake | In/out of model | Where |
|---|---|---|
| One JSON object per VEVENT, array output | in | core output shape |
| `uid`, `summary`, `start`, `end`, `location`, `description` | in | event fields |
| `status`, `categories` (as a list) | in | event fields |
| `organizer` + `attendees` (name + email) | in | parsed from CN param + `mailto:` value |
| **Recurrence (RRULE) parsed to a structured object** | in | `recurrence` (freq/interval/count/until/byday…) |
| All-day detection (`VALUE=DATE`) | in | `all_day` flag |
| ISO-8601 date normalization (default) | in | `date_format=iso` |
| Keep raw / epoch date forms | in | `date_format=raw|unix` |
| Pretty-print vs compact JSON | in | `pretty` toggle |
| Drop long descriptions | in | `include_description` toggle |
| Paste `.ics` text | in | page textarea |
| Worked example presets | in | page `[[example]]` chips |
| Copy result / reset | in | platform-provided page buttons |
| File / drag-drop upload | out | page is paste-based; state on page |
| Fetch `.ics` from a URL | out | this is a pure/no-network block; state on page |
| Full timezone **conversion** (TZID → target zone) | out | no tz database shipped; Z times exact, floating/TZID read as wall-clock — state on page |
| Reverse conversion (JSON → iCal) | out | separate tool |
| VTODO / VJOURNAL / VFREEBUSY parsing | out | only VEVENT is parsed — state on page |

## Design decisions

- Params kept tight: `ics` (required), `date_format` (iso/raw/unix), `pretty` (bool),
  `include_description` (bool). Recurrence, organizer, attendees, categories, and the
  `all_day` flag are always parsed and included only when present (empty fields are omitted so
  the JSON stays clean — mirrors `ics-to-csv`'s "column only when present").
- Every out-of-model item is listed above and surfaced in the page FAQ/limits, not built.
