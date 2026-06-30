# Improve time-to-decimal-hours — competitor analysis (2026-06-30)

## Scope

Tool: `time-to-decimal-hours`

Goal: convert clock durations (`H:MM` / `H:MM:SS`) to decimal hours and decimal hours back to canonical clock notation, with total minutes and seconds for timesheets, payroll, billing, and spreadsheet checks.

## Competitor scan

1. CalculatorSoup Time to Decimal Calculator
   - Strengths: common payroll-oriented conversion and examples.
   - Gaps closed here: bidirectional conversion in one result, JSON output, seconds support, negative durations, and local chat/CLI/page surfaces.

2. OnTheClock / payroll decimal hour converters
   - Strengths: simple HH:MM to decimal lookup for payroll.
   - Gaps closed here: decimal-to-clock conversion, flat total seconds/minutes, and no account/tracking context.

3. Redcort / Virtual TimeClock decimal hour calculators
   - Strengths: business-focused explanations and rounding tables.
   - Gaps closed here: deterministic exact conversion with explicit whole-second rounding for decimal inputs and machine-readable output.

4. OnlineConversion / unit-conversion sites
   - Strengths: general hours/minutes conversion.
   - Gaps closed here: familiar `H:MM[:SS]` input syntax, auto-detection, and payroll-ready one-line summary.

5. Spreadsheet formulas / custom scripts
   - Strengths: flexible inside a workbook.
   - Gaps closed here: no formula setup, validates bad minute/second components, and works consistently across gizza CLI, chat block, and browser page.

## In-model improvements included

- `auto`, `from-clock`, and `from-decimal` interpretation modes.
- Supports `H:MM`, `H:MM:SS`, signed durations, and decimal-hour inputs.
- Returns both representations plus decimal hours, total minutes, total seconds, and a human summary.
- Pretty JSON output for the browser page and CLI inspection.
- Clear parse errors for malformed components and minutes/seconds outside `0..=59`.
- Page SEO copy aimed at timesheet, payroll, and billing use cases.

## Out-of-model / not built

- Payroll policy rounding rules (nearest 6/10/15 minutes), multi-entry timesheet summing, and lunch/break subtraction are broader workflow tools and should be separate blocks.
- Time-of-day clock arithmetic across dates/time zones is covered better by date/time-specific tools.

## Verification checklist

- Core unit tests cover clock-to-decimal, decimal-to-clock, seconds, negative values, forced modes, invalid inputs, large durations, and JSON shape.
- Drift-guard schema test covers the chat/LLM descriptor.
- Web wrapper exposes `run(value, mode)` for the generated page.
- Playwright tests cover page conversion in both directions and query-param deep links.
