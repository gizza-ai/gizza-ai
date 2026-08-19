# first-difference-calculator — competitor analysis (2026-08-17)

Scan run BEFORE implementation, per `/create-next-tool` step 4. Everything below is
**paraphrased** from public documentation/teaching material — no competitor copy, branding, or
trademarks are reproduced, and no competitor asset is used.

## Who the real competitors are

There is no dominant single-purpose "first difference calculator" web tool; the function is
served by three overlapping groups. All five profiles below were reachable and are real tools
or reference implementations that a user would actually reach for.

| # | Competitor | What it is | Reached |
| - | ---------- | ---------- | ------- |
| 1 | pandas `Series.diff()` / `DataFrame.diff()` | the de-facto reference implementation for differencing a series | docs |
| 2 | pandas `pct_change()` | the percent-change sibling users pair with `diff()` | docs |
| 3 | R `base::diff()` | the other reference implementation, with an explicit `lag` + `differences` API | docs |
| 4 | Difference-transform tutorials (MachineLearningMastery "remove trends and seasonality") | the practitioner-facing "how do I difference my series" workflow | docs |
| 5 | Math-education first/second-differences material (Math Knowledge Network) + percentage-difference calculators (Omni Calculator) | the school-level "are these differences constant?" use, and the two-value percent tools people mistakenly use for series | docs |

## Profiles (paraphrased)

### 1. pandas `diff()`
- **Params:** `periods` (default 1) — how far back to subtract; `axis` (rows/columns).
- **Semantics:** output is **aligned to the input length**, with the first `|periods|` entries
  as missing values (NaN) rather than dropped.
- **Negative `periods`** compares each element with a **later** element instead of an earlier
  one (a lead rather than a lag); then the *trailing* entries are missing.
- **Higher orders** are done by chaining (`s.diff().diff()`), which naturally makes the null
  region grow.
- **UX:** none (library) — but the aligned-null convention is what every notebook user expects.

### 2. pandas `pct_change()`
- **Params:** `periods` (default 1), `freq`, `fill_method` (deprecated).
- **Formula:** `(current − previous) / previous`, returned as a **fraction**, not ×100.
- **Edge cases:** first rows missing; a zero baseline yields ±infinity; missing values are
  skipped unless forward-filled first.

### 3. R `base::diff()`
- **Params:** `x`, `lag` (default 1), `differences` (default 1, applied **recursively**).
- **Semantics:** output is **shorter than the input** — length `n − lag` for order 1, shrinking
  further per extra order. This is the opposite convention from pandas.

### 4. Difference-transform practitioner workflow
- **Params by another name:** *interval/lag* (1 for trend, = season length for seasonality —
  e.g. 12 for monthly data) and *order* (how many times to repeat until stationary).
- Recommends **seasonal differencing before trend differencing** when both are needed.
- Documents **inverting** a difference (`original(t) = differenced(t) + original(t−lag)`) as a
  first-class step, because you need the original scale back after forecasting.

### 5. Education / two-value percent calculators
- Teaches **first differences constant ⇒ linear relation**, **second differences constant ⇒
  quadratic relation**, with the explicit caveat that the input column must be evenly spaced
  and in order.
- Percent calculators (Omni-style) take exactly two values, show the absolute difference next
  to the percent, and spend real page space distinguishing percent **difference** (symmetric,
  divided by the mean) from percent **change** (directional, divided by the baseline). Their
  FAQs are mostly about that confusion.

## Table stakes → our decision (nothing dropped silently)

| # | Table stake (who) | Decision |
| - | ----------------- | -------- |
| 1 | `lag` / `periods`, default 1 (1,2,3,4) | **in-model** → `lag` param, default 1 |
| 2 | Negative lag = compare with a later value (1) | **in-model** → `lag` accepts negatives; trailing warm-up |
| 3 | `order` / `differences`, recursive (3,4,5) | **in-model** → `order` param, default 1, max 10 |
| 4 | Seasonal lag (12 for monthly) (4) | **in-model** → same `lag` param; documented + a preset chip |
| 5 | Percent change `(cur−prev)/prev` (2,5) | **in-model** → `mode = percent` (×100, signed) |
| 6 | Fraction/ratio form (2) | **in-model** → `mode = ratio` (cur/prev) |
| 7 | Log difference (practitioner staple for growth) (4) | **in-model** → `mode = log` (natural log ratio) |
| 8 | Aligned-null output (1,2) vs shorter output (3) | **in-model, both** → aligned nulls by default, `drop_warmup` for the R-style short form |
| 9 | Zero baseline → ±inf (2) | **improved** → reported as `null` + counted in `undefined`, never `inf` (JSON has no infinity) |
| 10 | Constant-difference ⇒ linear / quadratic reading (5) | **in-model** → `constant` flag + a plain-language `interpretation` line |
| 11 | Absolute difference shown next to the percent (5) | **in-model** → summary always reports both the delta stats and the mode used |
| 12 | Rounding/precision control (5) | **in-model** → `decimals`, default 6, slider 0–10 |
| 13 | Direction/up-down counts for period-over-period reporting (BI tools) | **in-model** → summary `increases`/`decreases`/`unchanged`, largest move + its index |
| 14 | Presets for common cases (5, BI tools) | **in-model** → `[[example]]` chips (monthly Δ, % change, seasonal lag 12, second differences) |
| 15 | Inverting a difference back to the original scale (4) | **out of scope for this tool** — it is the inverse tool (a cumulative/running-total transform), not a param of this one; `blocks/cumulative-percent-builder` already covers running totals |
| 16 | Charting the differenced series (Desmos-style) (5) | **out-of-model here** — the generic tool page renders text; `blocks/line-series-chart` is the charting surface |
| 17 | Multi-column / CSV column selection (1,3) | **out-of-model here** — single-series field by design; `blocks/csv-stats` and `blocks/downsample-timeseries` own column selection |
| 18 | Date-labelled rows (BI period-over-period) | **considered, rejected** — labels would double the parse surface for no computational gain; indices are reported instead so rows map back trivially |
| 19 | Stationarity tests (ADF/KPSS) to pick the order (4) | **out-of-model** — a statistical test suite, not a differencing calculator; would need a stats engine and belongs in its own tool |
| 20 | Accounts / saved series / cloud batch (BI tools) | **out-of-model** — gizza is browser-local, no account, no server |

## Gaps we close that the reference implementations do not

- One surface gives absolute, percent, ratio, and log differencing — pandas needs two calls,
  R needs hand-written arithmetic.
- Zero baselines degrade to `null` with a count instead of silent `inf`.
- The linear/quadratic reading is computed and stated, not left to the user to eyeball.
- Warm-up convention is a switch instead of a library-choice you have to live with.
