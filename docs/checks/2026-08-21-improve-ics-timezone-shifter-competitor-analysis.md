# ics-timezone-shifter — competitor analysis (2026-08-21)

Scan run BEFORE implementation, per `create-next-tool`. One WebSearch for the function
("shift/convert the timezone of every event in an .ics file"), then the top real tools were
skimmed. All notes are paraphrased observations of behaviour — **no competitor copy, wording,
branding or trademarks are reused anywhere in this tool**.

## Tools skimmed

| # | Tool | What it actually does for this job |
|---|------|-----------------------------------|
| 1 | "Convert iCalendar File (.ics) to new Timezone" — a widely-copied Python gist (icalendar + pytz) | CLI script, two args: the `.ics` file and a target timezone. Localizes each event's datetime to the calendar's original zone, then `astimezone()`s it to the target and rewrites the value. Also rewrites `DTSTAMP` and updates the calendar-level `X-WR-TIMEZONE`. |
| 2 | csvtoics.app (CSV → ICS converter, the closest shipping tool with an explicit timezone model) | IANA timezone picked from a dropdown, defaulting to the browser's zone. Two output modes: **TZID + an embedded `VTIMEZONE`** carrying the DST transitions (default), or **UTC `Z`-suffixed absolute timestamps** with no `VTIMEZONE`. Explicitly documents that all-day rows are unaffected by the zone choice, and that a `TZID` event "stays 9:00 local across DST". Long FAQ (≈14 entries) incl. a "why did my times shift?" troubleshooting entry. No hard file-size limit; single events only (no recurrence). |
| 3 | icsfile.com ICS editor (browser-based, client-side) | General `.ics` editor advertising edits to start/end dates, timezone, all-day flags and recurrence/exception rules. Marketing-level only — no documented timezone control, no statement about `VTIMEZONE` handling or DST edge cases. Claims client-side processing, "hundreds or thousands of events", 5 FAQ entries. |
| 4 (extra, unreachable pages replaced) | icalconverter.com tool suite | No dedicated shifter; instead several import/repair tools whose selling point is "fix timezone mismatches" and "fix Windows timezone identifiers" when moving a calendar between Outlook, Google and Apple. Confirms the second real user need: a calendar whose times are *labelled* with the wrong zone, not one that needs re-expressing. |

(anyonlinetool.com's ICS editor 404'd at fetch time and was replaced by icalconverter.com, per the
"replace an unreachable competitor, don't run with fewer" rule.)

## Table-stakes extracted → where each landed

| Table stake (seen at) | Decision | Where |
|---|---|---|
| Pick the **target IANA timezone** | in-model | `to` param, required, full IANA vocabulary via the page's `options = "timezones"` datalist, page default = the browser's zone (`default = "local-timezone"`) |
| Source zone for values that carry no zone of their own (1: reads `X-WR-TIMEZONE`; ours must not depend on a property most exports omit) | in-model | `from` param, default `UTC`, used for floating values and for unrecognized `TZID`s (e.g. Windows-style "Pacific Standard Time") |
| Rewrite `DTSTART` / `DTEND` (1, 3) | in-model | core rewrites `DTSTART`, `DTEND`, `DUE`, `RECURRENCE-ID`, `EXDATE`, `RDATE` and `RRULE`'s `UNTIL` |
| **TZID + embedded `VTIMEZONE`** output (2, default there) | in-model | `write_as = "tzid"` (default) + `include_vtimezone` (default on); the `VTIMEZONE` is generated from real DST transitions covering the years the events span |
| **UTC / `Z`-suffixed** output with no `VTIMEZONE` (2) | in-model | `write_as = "utc"` |
| Floating (zone-less) output — the third form real calendars use, not offered by any tool skimmed | in-model (differentiator) | `write_as = "floating"` |
| **Fix a mis-labelled export** — keep the wall clock, change the zone (4's "fix timezone mismatches", and the most common support question behind 2's "why did my times shift?") | in-model (differentiator) | `mode = "relabel"` alongside the default `mode = "convert"` |
| All-day (`VALUE=DATE`) events must not move (2 states this explicitly) | in-model | date-only values are passed through verbatim; stated on the page + in the FAQ |
| Correct DST handling across the transition (2) | in-model | `chrono-tz` IANA database; gap times roll forward one hour, ambiguous times take the earlier offset — both documented |
| Client-side / nothing uploaded (2, 3) | in-model | pure Rust → WASM, runs in the browser; stated in hero + FAQ |
| Preset / one-click examples (2, 3 both ship sample flows) | in-model | three `[[example]]` chips (UTC→Tokyo, relabel a mis-labelled export, TZID→UTC) |
| Stated capacity (2, 3 both address size) | in-model | 5000 events per run, stated on the page and enforced with a named error |
| Update the calendar-level `X-WR-TIMEZONE` (1) | in-model | rewritten to the target zone when the input carries one |
| Rewrite `DTSTAMP` too (1) | **rejected, deliberately** | `DTSTAMP`/`CREATED`/`LAST-MODIFIED` are defined as UTC instants; rewriting them (as 1 does) corrupts sync metadata. Left untouched, documented in Limits |
| Full `.ics` editing — retitle events, edit recurrence rules, add/remove events (3) | out-of-model | different tool; `ics-merge-dedupe`, `csv-to-ics` and `ics-parse` already cover neighbouring jobs |
| Expanding recurrences into individual instances (2 notes it can't either) | out-of-model | an `RRULE` master stays a master; only its `UNTIL` anchor is re-expressed |
| Drag-and-drop file upload / round-trip download of the `.ics` (2, 3) | partly out-of-model | this repo's pure-tool page takes pasted text; the generator already adds a Download link to `format = "text"` pages, so the result is downloadable. File-picker input for pure text tools is a platform-level change, not built here |
| Server-side calendar feed rewriting / subscription URLs (4) | out-of-model | network-fetch tool shape, not this one |

## Notes that shaped the implementation

- Both output models matter: a `TZID` event keeps its wall clock across a DST change, while a `Z`
  event is an absolute instant. Offering both, plus floating, is what makes the tool usable for
  imports *and* for server-side feeds.
- Tool 1's dependence on `X-WR-TIMEZONE` is its main weakness — most real exports don't set it, and
  the script then fails. Deriving the source zone per value (`Z` → UTC, known `TZID` → that zone,
  otherwise the `from` param) is strictly more robust.
- Emitting a `TZID` without a matching `VTIMEZONE` is technically non-conformant, which is why the
  generated `VTIMEZONE` (with real transitions for the covered years) is on by default.
