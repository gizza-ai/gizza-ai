# date-add-subtract competitor analysis — 2026-09-23

## Scope

Tool: `date-add-subtract` — add or subtract calendar and business-day durations from a date or datetime.

Model fit: pure Rust date arithmetic. In-model capabilities are date parsing, calendar month clamping, whole-unit offsets, configurable weekend calendars, holiday-date skipping, JSON/text output, CLI/page parity, validation and deterministic tests. Out-of-model capabilities are jurisdiction-specific holiday databases, timezone/DST conversion, recurring calendar events, and appointment scheduling integrations.

## Competitor scan summary

| Competitor pattern | Table-stakes behavior observed | In model? | Decision |
| --- | --- | --- | --- |
| General date calculators | Start date, add/subtract selector, years/months/weeks/days fields, weekday result, examples for 30/60/90 days | Yes | Implemented operation enum plus whole-unit fields and weekday/ISO/day-of-year output. |
| Business-day calculators | Option to skip weekends, holiday exclusions, due-date style examples | Yes | Implemented `skip_weekends`, configurable `weekend_days`, holiday list, skipped-day counters. |
| Deadline calculators | Month-end/leap-year correctness and clear interpretation of "one month" | Yes | Years/months apply first and clamp day-of-month to target month length. |
| Datetime calculators | Hours/minutes/seconds and rolling across day boundaries | Yes | Time units apply after calendar date math and return date/time fields. |
| Regional workday calendars | Country/state holiday presets and regional weekend defaults | Partial | Built manual holidays + weekend schedule enum. Preset public holiday databases are out-of-model without maintaining external data. |
| Timezone-aware calculators | DST, timezone offsets, local clock conversions | No | Out-of-model for this pure civil-date tool; documented as timezone-free arithmetic. |

## Required UX controls

- `date` text field with examples for ISO and relative words.
- `operation` enum/select: `add`, `subtract`.
- Number fields for years, months, weeks, days, hours, minutes, seconds.
- `skip_weekends` checkbox.
- `weekend_days` enum/select for Saturday/Sunday, Friday/Saturday, Thursday/Friday, single-day weekends and holiday-only mode.
- `holidays` multiline text area for comma/semicolon/newline-separated dates.
- Example chips for common use cases: 30 days from today, 90 days before a date, 1 year 6 months later, business days, holiday skipping and datetime shifts.

## Defaults and validation

- Default operation is `add`.
- Numeric blanks are treated as not supplied; explicit zero is valid.
- Units must be whole finite numbers; fractional dates are rejected with guidance to use a smaller unit.
- Large spans are bounded to avoid overflow. Business-day walking is capped at 200,000 working-day steps.
- Holiday list is capped at 1,000 dates.
- Inputs use naive civil time: no timezone conversion or DST adjustment.

## Worked examples covered

- `2026-06-19 + 90 days` → `2026-09-17` (Thursday).
- `2026-09-17 - 90 days` → `2026-06-19`.
- `2026-01-31 + 1 month` → `2026-02-28`.
- `2026-06-19 + 1 business day` with Saturday/Sunday weekend → `2026-06-22`.
- Holiday skipping with `2026-12-25, 2026-12-26, 2027-01-01`.
