# fiscal-quarter-mapper — competitor scan + build decisions (2026-08-20)

Scan run BEFORE implementing. All notes are paraphrased observations of what each tool
*does*; no competitor copy, wording, branding or trademark is reproduced or reused.

## Sources actually reached

| # | Tool / reference | What it is | Reached |
|---|---|---|---|
| 1 | Ultimate Finance Calculator — fiscal quarter start/end date calculator | single-date web calculator with a fiscal-year-start month selector | yes (full page) |
| 2 | ExtendOffice — "convert date to fiscal year/quarter/month in Excel" | the canonical spreadsheet recipe people use today | yes |
| 3 | pandas `Period.qyear` / `to_period('Q-JUN')` + datagy's pandas-fiscal-year walkthrough | the canonical *bulk column* approach — the closest analogue to this tool | yes (docs + walkthrough) |

Unreachable, replaced (403 / truncated at fetch time, 2026-08-20): globalcalcs fiscal-quarter,
beantoolbox date-quarter-calculator, freetoolscorner fiscal-year-calculator,
everycalculators quarter-calculator. Their search snippets agreed with #1 on the feature
set (fiscal-start presets, quarter start/end, progress, timeline), so #1 stands in for the
single-date-calculator class and #2/#3 cover the spreadsheet and dataframe classes.

## What each one does

**#1 single-date calculator.** Inputs: a date (defaults to today) and a fiscal-year start
month 1–12, offered with named presets — January = calendar year, April = UK/India,
July = Australia, October = US federal. Outputs: the fiscal quarter as `Q3 FY2026`, the
quarter's start and end dates, percent progress through the quarter, days remaining, the
next quarter, and the fiscal-year end date, plus a Q1–Q4 table with per-quarter day counts
and a visual timeline. Its worked example: 2026-04-25 with an October start → Q3 FY2026,
1 April – 30 June 2026, FY ends 30 September 2026. FAQ covers what a fiscal quarter is,
why companies use non-calendar years, and the 90–92 day spread.

**#2 spreadsheet recipe.** Three separate formulas — fiscal year, fiscal quarter, fiscal
month — each requiring the user to hand-build a 12-entry lookup table for their own fiscal
calendar. Notably it labels the fiscal year **by the calendar year the year BEGINS in**, and
it numbers the fiscal *month* 1–12 from the start month (July start → calendar July = fiscal
month 1). It is per-cell: nothing infers the column's date format, and mixed/ambiguous cells
are the user's problem.

**#3 dataframe approach.** `to_period('Q-JUN')` names the fiscal calendar by its **END**
month, and `.qyear` returns the **ending** year — so with a June-end year, July 2025 lands in
Q1 of FY2026. datagy shows the same and adds a hand-rolled lambda to render the year as a
`2020-2021` span. Bulk over a whole column, but requires Python, and the two year-naming
conventions (#2 start-year vs #3 end-year) silently disagree with each other.

## Table stakes → decisions

| Capability | Seen in | In model? | Decision |
|---|---|---|---|
| Configurable fiscal-year start month | 1, 2, 3 | in | `fiscal_start_month` enum of the 12 month names, default `january`; page `<select>` labels flag the Apr/Jul/Oct presets |
| Named presets (calendar / UK-India / Australia / US federal) | 1 | in | as `[input.labels]` on that select + `[[example]]` chips, not a second param |
| Quarter label `Q3 FY2026` | 1 | in | `quarter_label` enum, default `q-fy` |
| Quarter label `2026Q1` (pandas), `2026-Q1`, bare `Q1`, bare `1` | 3 | in | same enum: `yyyyqn`, `yyyy-qn`, `qn`, `n` |
| Fiscal-year labelled by END year | 1, 3 | in | `fiscal_year_naming = end` (default — US federal + pandas agree) |
| Fiscal-year labelled by START year | 2 | in | `fiscal_year_naming = start` — the disagreement is made an explicit switch instead of a silent convention |
| Year rendered `2025-2026` / `2025-26` span | 3 (hand-rolled) | in | `fiscal_year_label` enum: `range`, `range-short`, plus `fy-yyyy`, `yyyy`, `fy-yy` |
| Quarter start + end dates | 1 | in | `add_quarter_dates` checkbox → two ISO columns |
| Fiscal month / period 1–12 | 2 | in | `add_fiscal_month` checkbox |
| Quarter progress / days remaining | 1 | in, **redefined** | `add_quarter_position` → `day_of_quarter` + `days_in_quarter`, measured from the ROW's own date. "Days remaining until today" is meaningless for a historical row, so the deterministic day-N-of-M form ships instead; no clock is read anywhere |
| Whole-column bulk mapping | 3 only | in | the core of this tool — 1 and 2 are per-cell |
| Ambiguous `03/04/2024` handling | none | in | `date_order` auto/day-first/month-first, inferred column-wide from rows that can only be one thing; reported in the audit |
| Unreadable-cell policy | none (2 emits `#VALUE!`) | in | `on_error` = blank / drop / error |
| Audit of what was mapped | none | in | `output = report` / `json` |
| Q1–Q4 reference table, visual timeline, next-quarter, FY-end date | 1 | **out** | those are single-date-calculator chrome; a per-row CSV mapper has no single "current" quarter. The FY-end date is derivable from `add_quarter_dates` on a Q4 row |
| 4-4-5 / 52-53-week retail fiscal calendars | 3 (as a custom-calendar side note) | **out** | needs a per-company anchor week and a 53-week rule; a different tool, not a parameter here |
| Percent-progress bar / graphics | 1 | **out** | page renders text/CSV output |

## Copy / UX decisions taken from the scan

- The page must state the year-naming split explicitly with a worked example, because #2 and
  #3 quietly disagree and that is the single likeliest wrong answer a user ships.
- Preset chips for the four common fiscal calendars (calendar, UK/India April, Australia
  July, US federal October) — #1 proves presets are expected here.
- FAQ answers the questions #1's FAQ answers (what a fiscal quarter is, why a non-calendar
  year, why quarters differ in length) plus the two this tool has that they don't: which
  year-naming convention to pick, and what happens to an unreadable date.
