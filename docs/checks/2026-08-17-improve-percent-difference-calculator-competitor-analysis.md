# percent-difference-calculator — competitor analysis (2026-08-17)

Scan run **before** implementation, per `/create-next-tool` step 4. All findings are
paraphrased observations of publicly documented behaviour and formulas; no competitor copy,
branding, or trademarks are reproduced or reused.

## Scope

Backlog row: *"Computes absolute difference, percent change, and symmetric percent difference
between two values."* (`pure`)

Dup check: `blocks/percentage-calculator` already answers five percentage questions, one of which
is `change` (percent change `from` → `to`, plus `absolute_change`). It does **not** compute the
symmetric percent difference (`|a − b| ÷ mean`), which is a different formula with a different
reference point, and it is mode-switched so it can never show change and difference side by side.
The new tool is therefore a distinct calculator, not a semantic duplicate — every competitor below
also ships "percentage difference" and "percentage change" as separate pages for exactly this
reason.

## Competitors reviewed

1. **CalculatorSoup — Percentage Difference Calculator**
   (`calculatorsoup.com/calculators/algebra/percent-difference-calculator.php`)
   - Inputs: two plain numeric fields labelled V1 and V2, no defaults, no placeholders.
   - Output: one number — the percentage difference.
   - Formula published on-page: `|V1 − V2| / ((V1 + V2) / 2) × 100`.
   - Worked example: 5 and 7 → `|5 − 7| = 2`, mean 6, `2/6 × 100 = 33.33%`.
   - States the tool is intended for two positive numbers greater than zero, and demonstrates
     order-independence (7 and 5 gives the same 33.33%).
   - No rounding control, no sliders, no presets. Points users to a separate percentage-change
     calculator for directional change.

2. **Omni Calculator — Percentage Difference Calculator**
   (`omnicalculator.com/math/percentage-difference`)
   - Inputs: two value fields, plus a dropdown to switch between computing percentage *difference*
     and percentage *change*.
   - Outputs: the percentage difference **and** the plain absolute difference between the values.
   - Formula published on-page: `100 × |V1 − V2| / ((V1 + V2) / 2)`.
   - Worked examples: 70 and 85 → 19.355% difference with an absolute difference of 15; a
     step-by-step walkthrough of 20 and 30 → 40%.
   - Caveats stated: the measure is misleading when the two values differ by more than about one
     order of magnitude; the result hits exactly 100% only when one value is three times the other;
     the calculation cannot be inverted because of the absolute value.
   - Chrome: share-result, reload, clear-all buttons (site-wide framework features, not tool logic).

3. **Calculator.net — Percentage Calculator (difference + change sections)**
   (`calculator.net/percent-calculator.html`)
   - Percentage-difference section: two value fields, same published formula, worked example
     `|10 − 6| / ((10 + 6)/2) = 4/8 = 50%`.
   - Percentage-change section is a *separate* widget: a number, an increase/decrease toggle, and a
     percent — i.e. "apply a change", already covered by `blocks/percentage-calculator`.
   - No stated edge cases in either section.

## Table stakes → decisions

| Capability | Seen at | Verdict | Where it lands |
| --- | --- | --- | --- |
| Two numeric value inputs | all 3 | in-model | `a`, `b` params (required numbers) |
| Symmetric percent difference `|a−b| / |mean| × 100` | all 3 | in-model | always reported (`difference` + `all` modes) |
| Absolute difference `|a−b|` reported alongside | Omni | in-model | always reported |
| Published formula visible to the user | all 3 | in-model | formulas in `content.md` + in the output header line |
| Worked example with real numbers | all 3 | in-model | 70 vs 85 → 19.3548% in `content.md`; example chips on the page |
| Difference-vs-change mode selector | Omni (dropdown) | in-model | `mode` = `all` \| `difference` \| `change` (`Param::enumv`) |
| Directional percent change `(b−a)/|a| × 100` | Omni, Calculator.net | in-model | reported in `change` + `all` modes, both directions |
| Order-independence of the difference measure | CalculatorSoup, Omni | in-model | inherent to the formula; asserted by a unit test and documented in the FAQ |
| "Misleading across an order of magnitude" caveat | Omni | in-model | FAQ entry + a note emitted when the ratio exceeds 10× |
| Mean / midpoint shown | implied by all | in-model | reported as `Mean (a + b) / 2` |
| Decimal-place control | none of the 3 | in-model, **added** | `decimals` param (0–10, default 4) — competitors hard-round, which loses precision on small differences |
| Ratio `b / a` | none of the 3 | in-model, **added** | reported in `change` + `all` modes; cheap and the natural companion to percent change |
| Preset one-click examples | none of the 3 | in-model, **added** | `[[example]]` chips (`70 vs 85`, `5 vs 7`, `120 vs 100`, `-4 vs 6`) |
| Share-result / reload / clear-all chrome | Omni | out-of-model here | the generator already gives every page Reset + Copy result; deep-linkable `?a=&b=` covers sharing |
| Feedback widgets, ads, related-calculator link farms | Omni, Calculator.net | out-of-model | site-repo concerns, not toolkit concerns |
| Unit/currency pickers | not shipped by any of the 3 | out-of-model | the measure is unit-free; a unit label would be decoration only |
| Step-by-step rendered derivation as HTML/MathML | Omni (prose) | out-of-model | `content.md` carries the derivation statically; per-run HTML math is a page-renderer feature |

## Edge cases the competitors leave undefined (we define them)

CalculatorSoup restricts itself to "two positive numbers"; none of the three state what happens at
zero, with negatives, or with values that cancel. Decisions taken here:

- **`a + b == 0`** (e.g. `5` and `-5`): the mean is zero, so percent difference is undefined.
  In `all` mode it is omitted with an explanatory note; in `difference` mode it is a hard error.
- **`a == 0`**: percent change from `a` and the ratio `b / a` are undefined (division by zero).
  Omitted with a note in `all` mode; a hard error in `change` mode.
- **`b == 0`**: the reverse percent change `b → a` is undefined; omitted with a note.
- **Negative values**: the mean's absolute value is used as the reference (`|a − b| / |mean|`), so
  `-70` vs `-85` gives the same 19.3548% as `70` vs `85`. Percent change likewise divides by `|a|`,
  which keeps the sign of the change equal to the sign of `b − a` rather than flipping on a
  negative baseline. Both conventions are documented on the page.
- **Non-finite input** (`NaN`, `inf`): rejected with a message naming the offending field.
- **Order-of-magnitude warning**: when the larger magnitude exceeds 10× the smaller, a note repeats
  Omni's caveat in our own words rather than silently returning a near-200% figure.

## Not built (recorded, deliberately out of scope)

Share/reload/feedback chrome, ads and related-tool cross-links, unit or currency pickers, and a
dynamically rendered step-by-step math derivation. None affect the computation; the first two are
site-repo responsibilities and this repo renders generic, brand-free pages.
