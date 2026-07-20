# calendar-freebusy-overlap — competitor analysis (2026-07-20)

Scanned before implementation (build-time scan, per create-next-tool step 4).
Function: paste two iCalendar (.ics) files → list the time windows where BOTH
calendars are free. All notes paraphrased; no competitor copy/branding reused.

## Competitors reviewed

1. **Overlap** (overlap-timeline.vercel.app) — paste an .ics or subscribe to a
   calendar URL; renders a shared timeline. Controls: date-range presets
   (7d/14d/30d), home timezone, per-zone working hours, layer filters
   ("free for all" / "off-hours for someone" / "double-booked"), anonymize
   titles, drag-to-create, click-to-book.
2. **WU Tools Meeting Time Planner** (wutools.com/time/meeting-time-planner) —
   participants entered by hand (no ICS): per-person IANA timezone + working
   hours (default 09:00–18:00), meeting-duration presets (30/45/60/90/120 min),
   fixed 7-day forward scan at 1-hour granularity, ranked suggestion list with
   quality markers, copy + per-slot .ics download.
3. **Elysia Date Overlap Checker** (elysiatools.com) — textarea of
   `name, start, end` date ranges, single Run button, text result naming which
   ranges overlap; date-only (no timestamps); FAQ covers boundary rules +
   local-only processing.

## Table stakes → decision

| Capability | Competitors | Fit | Decision |
| --- | --- | --- | --- |
| Paste .ics text (2 calendars) | Overlap | in-model | two required multiline fields `calendar_a`/`calendar_b` |
| Date range to scan, presets 7/14/30 days | Overlap, WU | in-model | `start_date` (date picker, default today) + `days` 1–60 (default 7) + example chips |
| Working hours window | Overlap, WU (09:00–18:00) | in-model | `day_start`/`day_end` time pickers, default 09:00–17:00 |
| Minimum meeting duration (30/45/60/90/120) | WU | in-model | `min_minutes` integer 5–720, default 30, presets via example chips |
| Timezone selection (IANA) | Overlap, WU | in-model | `timezone` with the shared timezones vocabulary, page default = local timezone |
| Weekday/weekend handling | implied by "working hours" | in-model | `weekends` boolean, default false (weekdays only) |
| Ranked/structured result list | WU | in-model | per-day slot list with weekday, local times, duration + totals |
| Export result as .ics | WU (per-slot download) | in-model | `output = "ics"` renders an RFC 5545 VFREEBUSY with FBTYPE=FREE periods |
| Machine-readable output | Elysia (API-ready) | in-model | `output = "json"` |
| Recurring events (weekly standups etc.) | implied — real calendars have them | in-model (scoped) | RRULE FREQ=DAILY/WEEKLY(+BYDAY)/MONTHLY(+single BYMONTHDAY)/YEARLY with INTERVAL/COUNT/UNTIL + EXDATE; caps + limits stated on page |
| Busy semantics | implied | in-model | skip STATUS:CANCELLED + TRANSP:TRANSPARENT; VFREEBUSY BUSY periods honored |

## Out-of-model (listed, not built)

- **Calendar URL subscription / auto-resync** (Overlap): the pure block has no
  network access; paste-only. (CLI users can `curl` the feed themselves.)
- **Google/Outlook/CalDAV account connection** (Overlap, YouCanBookMe,
  Syncdate): OAuth integrations are a hosted-service concern.
- **Visual timeline/heatmap grid + drag-to-book** (Overlap, Morgen): the page
  is a text-first tool; a slot list covers the job. Deferred as a possible
  custom.js enhancement.
- **Group polls / invitee voting** (Morgen, Timeful, Carly): multi-user state.
- **Per-participant timezones** (WU): both pasted calendars carry their own
  event timezones already (TZID handling is in-model); a single *output*
  timezone is the meaningful knob here.
- **Booking links / emailing invites** (YouCanBookMe): hosted-service concern.

## Spike notes

- ICS parsing hand-rolled (line unfolding + property params); no wasm-risk dep.
- `chrono-tz` proven in `blocks/timezone-convert` (pure-Rust IANA db, DST
  correct) — used for TZID + working-hours materialization.
- RRULE expansion spiked as wall-clock arithmetic in the event's zone with an
  iteration cap; unknown TZIDs (e.g. Windows zone names) fall back to the
  selected timezone and this limit is stated on the page.
