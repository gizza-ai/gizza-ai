# decimal-to-fraction — competitor analysis (2026-09-23)

Scan run BEFORE implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of publicly visible behaviour; no competitor copy, wording, branding, or trademarks
are reproduced or reused anywhere in the block.

Backlog row: `decimal-to-fraction | math | Approximates a decimal as the simplest fraction within a
tolerance using continued-fraction convergents. | pure`

Dup check: `ls blocks/ | grep -iE 'fraction|decimal|ratio|percent'` → no fraction block exists.
`round-decimals` (decimal-place rounding), `percent-decimal-converter` (percent ↔ decimal column
conversion) and `aspect-ratio-validator` (video display ratios) are all different jobs. Not a dup —
built.

## Competitors reviewed

| # | Tool | Inputs / options | Outputs | Notes |
|---|------|------------------|---------|-------|
| 1 | CalculatorSoup — Decimal to Fraction | decimal (+/− toggle); "how many trailing decimals repeat" count; "round to the nearest fraction" dropdown (thirds, eighths, twentieths, hundredths, …) | improper fraction + mixed number, step-by-step work (put over 1 → multiply out decimal places → GCF → reduce) | Richest option set of the five. Repeat count and the nearest-fraction snap are the two real knobs. |
| 2 | Omni Calculator — Decimal to Fraction | decimal; repeating-digits field; reload/share | simplified fraction, mixed-number conversion explained | Auto-reduces via GCD. Worked examples 0.125 → 1/8, repeating 0.6(25) → 619/990, 1.8(3) → 11/6. |
| 3 | Inch Calculator — Decimal to Fraction | single decimal field | fraction, mixed number, step-by-step solution, ~30-row decimal↔fraction reference chart | No tolerance/denominator options exposed; leans on the worked steps + chart. |
| 4 | The Calculator Site — Decimal to Fraction | decimal; "trailing decimal places to repeat" (default 0) | fraction + step-by-step method | States explicitly that irrationals (π) have no fraction — a limits note worth matching. |
| 5 | GigaCalculator — Decimal to Fraction | single decimal field | mixed fraction AND simple fraction; 20-row conversion table (0.01–1.00) | Documents the ×10^N / GCD method; no repeat handling visible. |

Sources: calculatorsoup.com, omnicalculator.com, inchcalculator.com, thecalculatorsite.com,
gigacalculator.com (decimal-to-fraction pages, fetched 2026-09-23).

## Table stakes → where each one landed

| Table stake | Seen on | Decision | Where |
|---|---|---|---|
| Plain decimal → reduced fraction | all 5 | in-model | `decimal` (required); always reduced via GCD |
| Negative decimals, `+` sign, integers | 1, 2, 5 | in-model | parser handles sign, unicode minus, bare integers |
| Mixed-number output | 1, 2, 3, 5 | in-model | `mixed_number`, `whole_part`, `proper_numerator/denominator` in the JSON |
| Improper fraction shown alongside | 1, 5 | in-model | `fraction` + `numerator`/`denominator` |
| Repeating decimals by digit count | 1, 2, 4 | in-model | `repeating_digits` (0 = none) |
| Repeating decimals by notation | 2 (0.6(25) style) | in-model, extra | parser also accepts `0.1(6)`, `0.1[6]`, `0.16…`/`0.16...` |
| Step-by-step work | 1, 2, 3, 4, 5 | in-model | `steps[]` — over-1 setup, ×10^N, GCD, reduce; algebra steps for repeats; convergent steps for approximations |
| Round to the nearest 1/N ("nearest fraction" dropdown) | 1 | in-model | `denominator` (0 = off; e.g. 16 for sixteenths) + `rounding` = nearest\|up\|down |
| Keep the snapped denominator unreduced (8/16 vs 1/2) | 1 (implicit in its /100, /20 modes) | in-model | `reduce` boolean (default true) |
| Simplest fraction within a tolerance (the row's core promise) | none of the five | in-model, differentiator | `tolerance` (absolute error budget) via a Stern-Brocot simplest-in-interval search |
| Cap the denominator | none of the five | in-model, differentiator | `max_denominator` — best approximation with q ≤ cap, from continued-fraction convergents + semiconvergents |
| Exactness / error reporting | none of the five | in-model, differentiator | `is_exact`, `fraction_value`, `error`, `error_percent`, `exact_fraction` |
| Continued-fraction convergent ladder | none of the five | in-model, differentiator | `convergents[]` (0.142857 → 1/7, 1/7 after 0/1) |
| Decimal ↔ fraction reference chart | 3, 5 | in-model (page copy) | common-conversions table in `page/content.md` |
| Preset buttons / one-click examples | 1 (dropdown presets) | in-model | `[[example]]` chips: eighths, sixteenths, repeating, π ≈ 355/113, tolerance |
| Percent input (`12.5%`) | not seen | in-model, cheap | parser divides by 100 |
| Scientific notation (`1.25e-3`), thousands separators | not seen | in-model, cheap | parser normalises both |
| Copy result / share / reset / deep link | 1, 2, 3, 5 | already platform | generator gives Copy, Reset and `?param=` deep links for free |

## Out of model (listed, not built)

- **Rendered fraction typography / stacked bar + vinculum over the repeating digits.** The page
  output is a JSON text block; a typeset `0.1̄6` glyph run and stacked numerator/denominator layout
  would need a bespoke `custom.js` renderer. Deferred, not silently dropped.
- **Social share / embed widgets, citation generators, ad-free/dark-mode chrome.** Site-repo
  concerns; this repo renders generic unbranded pages.
- **Interactive lesson content / video walkthroughs** (Omni's 90-second explainer). Editorial, not
  compute.
- **Irrational "exact" forms** (recognising 0.7071 as √2/2, 3.1416 as π). A symbolic-constant
  recognizer is a different tool; the tool instead states the limit on the page and returns the
  best rational approximation.
- **Fraction → decimal, fraction arithmetic, common-denominator rewriting.** Separate backlog rows
  (`fraction-to-decimal-converter`, `fraction-calculator`, `common-denominator-converter`); out of
  scope here to avoid overlapping blocks.

## Defaults chosen

`repeating_digits = 0`, `tolerance = 0`, `max_denominator = 0`, `denominator = 0`,
`rounding = nearest`, `reduce = true` — i.e. the default run is an **exact** conversion of the digits
as typed, which is what all five competitors do by default. The approximation knobs are opt-in and
surfaced through the example chips.

## Limits stated on the page

Up to 18 fractional digits and 6 repeating digits; `max_denominator` and `denominator` capped at
1e15 / 1e12; integer overflow during the search is reported as an error rather than wrapping;
irrational values can only be approximated, never converted exactly.
