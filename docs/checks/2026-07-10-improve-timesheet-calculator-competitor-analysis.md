# timesheet-calculator — competitor analysis (2026-07-10)

Scan performed before implementing. One web search ("timesheet calculator work hours billable rate
total online tool"); skimmed the top real competitor tools below. All notes are paraphrased — no
competitor copy, branding or trademarks reproduced.

## Competitors skimmed

1. **Toggl — Billable Hours Calculator** (toggl.com/online-tools/billable-hours-calculator) — weekly
   grid of clock-in/clock-out per day; handles breaks, overtime and totals; a separate "6-minute"
   billing mode that logs time in tenths of an hour and separates billable vs non-billable per
   matter.
2. **Clockify — Time Card Calculator** (clockify.me/time-card-calculator) — per-day in/out rows with
   an optional break column; a "calculate pay" toggle that multiplies total hours by an hourly rate;
   supports overtime after a daily/weekly threshold.
3. **Rize — Timesheet / Time Card Calculator** (rize.io/tools/timesheet-calculator) — total work
   hours and gross pay from hours + rate; billable-revenue view from rate × utilization; downloadable
   weekly timesheet / CSV export.
4. **MemTime — Billable Hours Calculator** (memtime.com/billable-hours-calculator) — timesheet mode
   with daily totals and CSV export; a 6-minute increments mode for many small tasks; scenario
   modelling (discounts, different rates).
5. **Redcort / CalculateHours — Free Timecard Calculator** — manual weekly grid, totals hours and
   minutes, optional lunch/break deduction, converts to decimal hours for payroll.

## Table-stakes parameters / features

| Feature | Competitors | In-model here? | Decision |
|---|---|---|---|
| Start/end time per entry | all | ✅ | `log` line `START-END` |
| 24-hour AND am/pm times | all | ✅ | `parse_time` accepts both |
| Overnight / past-midnight shift | Clockify, Toggl | ✅ | end < start rolls +24h |
| Total hours (and minutes) | all | ✅ | `total_hours`, `total_minutes`, per-entry `hours` |
| Decimal-hours output | Redcort, Rize | ✅ | `hours` fields are decimal |
| Hourly rate → pay/billable amount | Clockify, Rize, Toggl | ✅ | `rate` + per-entry/total `amount` |
| Per-project / per-client grouping | Toggl (per matter), MemTime | ✅ | project tag → `projects[]` rollup |
| Per-project rate overrides | MemTime (scenario rates) | ✅ | `rates` = `Project=amount` overrides |
| 6-minute / 0.1h billing increment | Toggl, MemTime (legal) | ✅ | `round=6` (+10/15/30/60) |
| 15/30/60-min payroll rounding | Redcort, Clockify | ✅ | `round` enum |
| Currency symbol on amounts | Rize, Clockify | ✅ | `currency` |
| Notes / description per entry | Toggl | ✅ | free text after project, echoed as `notes` |
| Break/lunch deduction column | Clockify, Redcort | ⚠️ out-of-model (single-field log) | Documented workaround: split the day into two entries around the break (no in/out/break grid on a text page). |
| Overtime (>8h/day or >40h/wk premium) | Clockify, Toggl | ⚠️ out-of-model | Not a pure per-entry compute (needs day/week bucketing + a premium multiplier policy); left out to keep the model deterministic and simple. |
| CSV / timesheet export | Rize, MemTime | ⚠️ out-of-model | Page returns structured JSON that copies cleanly; a CSV export button is a generic page feature, not built per-tool here. |
| Utilization % → revenue | Rize | ⚠️ out-of-model | Different calculator shape (capacity planning), not a work-log totaller. |

## Design decisions

- **Freeform text log, not a fixed weekly grid.** The gizza page is a single form; a plain-text log
  (`START-END PROJECT notes`, one per line) is the most expressive single-field input and matches how
  freelancers keep notes. It covers arbitrary numbers of entries, dates and projects.
- **Project grouping is the differentiator** vs the simpler time-card calculators: entries tagged
  with the same project (with or without a leading `#`) roll up into per-project hour + amount totals
  plus a grand total.
- **Rounding + per-project rates** cover the legal-billing (6-min) and multi-client (different rate
  per client) cases that Toggl/MemTime call out.
- **Out-of-model features are documented, not silently dropped:** break deduction (split the entry),
  overtime premiums, CSV export and utilization revenue are listed above with the reason each is
  outside a deterministic per-entry text calculator.
- No competitor copy, wording, branding or example data was copied; the tool's format, field names,
  labels and FAQ are original.
