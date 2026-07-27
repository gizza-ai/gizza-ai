# round-to-nearest-multiple — competitor analysis (2026-07-25)

Pre-implementation scan of the top real "round to nearest multiple" tools. Findings are
paraphrased; no competitor copy, branding, or trademarks are reproduced.

## Competitors skimmed

1. **CalculatorSoup — Round to Nearest Multiple** (calculatorsoup.com) — single number + a
   multiple; rounds to nearest up or down, "similar to Excel MROUND()". Demonstrates common
   multiples 10 / 5 / 1 / 0.25 / 0.10 / 0.05 / 0.01. Worked example: 76.525 to nearest 0.05 →
   76.55. Accepts two-positive or two-negative pairs. No explicit tie-break documentation.
2. **GraphCalc — Round to Nearest Multiple Calculator** (graphcalc.com) — the most complete.
   Single value OR a **batch list** of numbers. Rounding *direction*: nearest, up (ceiling),
   down (floor), toward 0, away from 0. **Tie-break** options: half-up, half-down, half-even
   (banker's on the quotient), half-away-from-0, half-toward-0. Formatting toggles: thousands
   separators, decimal-places display (0–10). Formula stated as `Multiple × Round(Number ÷
   Multiple)`. Examples: 47→5=45, 82→10=80, 137→25=125, 276→50=300. Use cases: finance, retail
   pricing, construction, education, CS, sports.
3. **EverydayCalculation — Round up/down to multiple** (everydaycalculation.com) — free tool to
   round a number to any multiple, with explicit **round up / round down / round to nearest**
   direction buttons. (Page 403'd the fetcher on this run; captured from the search snapshot,
   which is why it replaced the also-403 CalcSure result.)

Common formula across all three: `result = multiple × round(value ÷ multiple)`, with the
rounding of the quotient varying by direction/tie mode.

## Table-stakes → decision (in-model unless noted)

| Capability | Decision |
|---|---|
| Round to nearest multiple (core) | **in** — `step` param + `half_up` default mode |
| Arbitrary step incl. decimals (0.25, 0.05, 1000) | **in** — `step` is a `number`, validated `> 0` |
| Round **up** to next multiple (ceiling) | **in** — `mode = ceil` |
| Round **down** to previous multiple (floor) | **in** — `mode = floor` |
| Round **toward 0** | **in** — `mode = truncate` |
| Tie-break half-up / half-down / half-even | **in** — `mode = half_up / half_down / half_even` |
| **Batch** many numbers at once | **in** — operates over a whole CSV/list, every numeric cell |
| Negative numbers | **in** — signed handling in the core, tested |
| Preset multiples (5, 0.25, 100, 1000…) | **in** — one-click `[[example]]` chips |
| Fixed-width output (pad to step's decimals, e.g. 1.00 / 1.25) | **in** — `trailing_zeros` toggle |
| Choose columns / delimiter / header for tabular data | **in** — mirrors sibling `round-decimals` |
| **Away-from-0** direction & half-away/half-toward extra tie modes | **omitted (minor)** — the 6-mode set (half_up/half_down/half_even/ceil/floor/truncate) already covers nearest+up+down+toward-0; away-from-0 is a rarely-used fifth direction. Listed here, not built, to keep the enum tight. |
| Thousands-separator display toggle | **omitted (formatting-only)** — output stays machine-parseable plain numbers; separators would collide with the CSV comma delimiter. |
| Decimal-places display override (0–10) | **covered differently** — output precision follows the step's own decimal places; `trailing_zeros` gives fixed width. No separate rounding-then-reformat step is needed. |

## Defaults chosen

- `step = 1` (round to whole numbers by default), `mode = half_up` (classical / MROUND-style),
  `header = true`, `delimiter = ","`, `columns = ""` (every numeric cell), `trailing_zeros =
  false`. These match the least-surprising spreadsheet defaults.

## Worked examples to ship (as page example chips + content.md)

- Prices to the nearest **0.05**: `1.23 → 1.25`, `2.42 → 2.40` (half_up).
- Values to the nearest **5** / **1000** (round_up vs round_down direction).
- Nearest **0.25** with fixed width (`trailing_zeros`) → `1.00 / 1.25 / 1.50`.
- Negative + banker's tie handling on the quotient.
