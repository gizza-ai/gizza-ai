# age-calculator — competitor analysis (2026-06-22)

## Tool

`gizza tool age-calculator birthdate=<dob> [as_of=<date>]` — compute a person's
exact age from a birthdate as of a target date (today by default). Surfaces:
chat skill (wafer block), CLI, and the `/tools/age-calculator/` page. Pure
compute, no network, runs client-side.

## Surfaces verified

- **Chat block** — `wafer build` validates instantiation (377 KiB); the
  `Utc::now` clock import resolves "today" in-runtime. Drift-guard test
  (`schema_json_matches_authored_chat_schema`) passes; the chat schema matches
  `manifest.json`.
- **CLI** — `birthdate=2000-06-22 as_of=2026-06-22`, default-today (omit
  `as_of`), and the negative-age error path all return correct JSON.
- **Page** — 3 Playwright specs pass: calendar breakdown + zodiac + weekday;
  totals + next-birthday countdown; and the blank-`as_of` browser-clock path.
- **Core unit tests** — 13 tests (breakdown, Jan-31 month clamp, Feb-29 leap
  handling, totals, zodiac boundaries, Chinese zodiac + generation cycle,
  flexible parsing, future-birthdate error, JSON shape).

## Top competitors surveyed

1. calculator.net / Almanac Age Calculator
2. agecalc.org
3. onlineagecalculator.app
4. dqydj.com age calculator
5. fcconvert.com / pulsafutura.com (zodiac + numerology angle)

Sources:
- https://www.almanac.com/tool/age-calculator
- https://agecalc.org/
- https://www.onlineagecalculator.app/
- https://dqydj.com/age-calculator/
- https://fcconvert.com/birthday-calculator
- https://www.pulsafutura.com/age-calculator-zodiac-signs-birthday-countdown/

## Feature gap analysis

| Capability | Competitors | gizza age-calculator | Status |
|---|---|---|---|
| Age in years / months / days (calendar) | yes | yes | ✓ matched |
| Correct leap-year + variable month length | yes | yes (calendar stepping, not /seconds) | ✓ matched |
| Total months / weeks / days | yes | yes | ✓ matched |
| Total hours | yes | yes | ✓ matched |
| Total minutes / seconds | some | **added** | ✓ closed |
| "As of" arbitrary target date | yes | yes (defaults to today) | ✓ matched |
| Next birthday + days countdown | yes | yes | ✓ matched |
| Day of week born | yes | yes (`%A`) | ✓ matched |
| Western (sun) zodiac sign | most | yes | ✓ matched |
| Chinese zodiac animal | some | **added** (year-cycle lookup) | ✓ closed |
| Generation label (Boomer/X/Millennial/Z/Alpha) | some | **added** | ✓ closed |
| Flexible input formats (ISO, RFC-3339, US, EU) | partial | yes | ✓ matched / better |
| Real-time recompute as you type | yes | yes (page driver) | ✓ matched |
| Private / offline / client-side | varies | yes (wasm, no upload) | ✓ better |

### Gaps intentionally NOT built (out of model / scope)

- **Age on other planets**, **numerology**, **"fun milestone" countdowns**
  (e.g. 10,000-days party) — novelty/entertainment features; low signal, and
  numerology is pseudo-scientific. Not in scope.
- **Age-difference between two people** — a distinct two-person comparison tool;
  the existing `date-diff` block already computes the duration between any two
  dates, so this would overlap. Left to `date-diff`.
- **Chinese New Year exact boundary** for the Chinese zodiac — competitors that
  show Chinese zodiac use the simple Gregorian-year mapping; matching that
  convention (documented in the code comment) rather than computing the lunar
  new-year cutoff, which would add a lunar-calendar dependency for marginal gain.

## Outcome

All in-model competitor capabilities are covered. Closed three real gaps this
pass (total minutes/seconds, Chinese zodiac, generation label) — additive
output fields only, so no chat-schema drift. No competitor copy, branding, or
trademarks were used.
