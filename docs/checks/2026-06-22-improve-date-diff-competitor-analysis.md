# date-diff — competitor analysis (2026-06-22)

## Tool summary

`date-diff` computes the duration between two dates/datetimes. Three surfaces:

- **Chat skill** (`gizza-ai/date-diff`): args `start`, `end` → `{ result: { years, months,
  days, hours, minutes, seconds, total_weeks, total_days, total_hours, total_minutes,
  total_seconds, summary, from, to, negative } }`.
- **CLI**: `gizza tool date-diff start=2020-01-15 end=2022-03-20` → flat JSON.
- **Page** `/tools/date-diff/`: two text fields → pretty-printed JSON breakdown.

Pure-Rust (`chrono`, `serde`), runs on every backend incl. the chat Service Worker.

## Competitors reviewed (top 5)

1. **timeanddate.com — Date Duration Calculator** — the category leader. Breakdown in
   years/months/weeks/days; "include end day" toggle; optional business-days; total counts
   in each unit; weekday names.
2. **calculator.net — Date Calculator** — combined Y/M/W/D breakdown + separate totals;
   add/subtract-days mode; business-days with US holidays.
3. **calculatorsoup.com — Date Difference Calculator** — days-only total; business-days mode
   (with "Saturday is a business day"); broad input-format support (US/EU/ISO).
4. **gigacalculator.com — Days Between Dates** — days/weeks/months/years; business-days.
5. **convertunits.com / html-code-generator** — days + weeks + months breakdown, simple.

## Gap analysis (fit-to-model)

| Capability | Competitors | date-diff | Verdict |
|---|---|---|---|
| Calendar breakdown Y/M/D | all | ✅ Y/M/D **+ H/M/S** | met / exceeded |
| Flat totals per unit (weeks/days/hours/min/sec) | timeanddate, calculator.net | ✅ all five | met |
| Time-of-day precision (hours/min/sec) | timeanddate (datetime) | ✅ | met (many competitors are date-only) |
| Multiple input formats (ISO, US, EU, slash, dot, RFC-3339) | calculatorsoup, calculator.net | ✅ ISO/US/`YYYY/MM/DD`/`DD.MM.YYYY`/RFC-3339 | met |
| Leap-year / variable-month correctness | all (claimed) | ✅ stepped-calendar, unit-tested (Jan31→Mar31 = 2mo; Feb29 span) | met |
| Human-readable summary string | timeanddate | ✅ `"2 years, 2 months and 5 days"` | met |
| Reversed-order handling | some | ✅ positive magnitude + `negative` flag | met |
| "Include end date" toggle (±1 day) | timeanddate, calculator.net | ❌ | in-model, deferred — minor convenience; our model is exact-instant duration which is unambiguous. Could add a `boolean` param later. |
| Business-days-only count | calculatorsoup, calculator.net, giga | ❌ | in-model (pure weekday counting) but a **distinct tool shape** (would need its own param + arguably its own block); not part of "duration between two dates". Out of scope for this tool. |
| Holiday-aware business days | calculator.net | ❌ | out-of-model — needs a per-country holiday dataset; explicitly not building. |
| Calendar date-picker widget | most | ❌ | UI nicety; the page uses plain text fields by design (works offline, paste-friendly, accepts all formats). Out of model for the generator's field renderer. |
| Add/subtract days (date arithmetic) | calculator.net | ❌ | a different tool (date math, not diff) — separate backlog item. |

## Decisions

- **Shipped the superset of the core duration feature**: unlike calculatorsoup (days-only) and
  several others, date-diff returns BOTH a mixed calendar breakdown down to seconds AND flat
  totals in five units, plus a grammatically-correct summary — matching/exceeding timeanddate
  and calculator.net on the duration use case.
- **Not built (by design):** business-days mode and date-arithmetic are distinct tool shapes
  (separate backlog candidates), holiday-aware counting needs an out-of-model dataset, and the
  calendar-picker is a generator-level UI feature. The plain text fields accept every common
  format including pasted RFC-3339 timestamps, which is the practical equivalent.
- **No competitor copy, branding, or trademarks were used.**

## Verification (all green)

- `cargo test --workspace` in `blocks/date-diff` — 14 tests (13 core + 1 schema drift guard) pass.
- `wafer build` — block.wasm validates/instantiates (368 KiB).
- `wasm-pack build` — web pkg built.
- CLI: `gizza tool date-diff …` returns correct breakdown + totals for date and datetime inputs.
- Page: `tests/tool-page-date-diff.spec.ts` — 3 Playwright tests pass (breakdown, datetime+totals,
  reversed-order flag).

## Sources

- https://www.calculatorsoup.com/calculators/time/date-difference-calculator.php
- https://www.calculator.net/date-calculator.html
- https://www.timeanddate.com/date/duration.html
- https://www.gigacalculator.com/calculators/days-between-dates-calculator.php
- https://www.convertunits.com/dates/
