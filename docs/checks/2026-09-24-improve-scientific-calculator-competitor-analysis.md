# scientific-calculator — competitor analysis (2026-09-24)

Scan run **before** implementation, per `/create-next-tool` step 4. All findings are paraphrased
from public feature descriptions; no competitor copy, branding, or trademarks are reproduced, and
no competitor asset is used. Out-of-model items are listed, not built.

## Why this is not a duplicate of `blocks/calculator`

`blocks/calculator` is a 43-line `meval` wrapper: one `expr` param, one `f64` result, page
`format = "number"`. It cannot express the three capabilities in this backlog row —
**variables**, **complex numbers**, and **arbitrary precision** — and its double-only `f64`
result type cannot represent any of them (`sqrt(-1)` → non-finite error, `2^200` → rounded
double, no assignment/`ans` state). The already-skiplisted `math-expression-evaluator` row was a
true duplicate because its scope was exactly calculator's (sqrt/parens/`pi`/`e`); this row is not.
Basic-vs-scientific are also distinct pages on every competitor scanned.

## Competitors scanned

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | devtools.tools — scientific calculator | yes | 64-digit precision, complex, `A = 42` assignment, `ans`, deg/rad |
| 2 | infinitycalculator.com — arbitrary-precision calculator | yes | 1–2000 significant digits, 10 variables, rational/fixed/scientific output, digit grouping |
| 3 | calculators-math.com — scientific / complex calculator | yes | full complex function set, rectangular + polar/phasor output, deg/rad/grad |
| 4 | web2.0calc.com | yes | precision presets (auto/12/18/24/30/48/60), complex, deg/rad, standard vs scientific notation |
| 5 | alcula.com — scientific calculator | yes | fractions, complex via `cis`, fixed/float/scientific/engineering display, `ans` |

## Table stakes → decisions

| Table stake | Seen at | Decision | Where it landed |
|---|---|---|---|
| `+ - * / ^ ( )`, unary minus | all 5 | **in-model** | operator table |
| Modulo (`%` / `mod`) | 1, 2 | **in-model** | `%` operator + `mod(a,b)` |
| Factorial `!` | 1, 2, 5 | **in-model** | postfix `!` + `fact(n)` |
| Implicit multiplication (`2pi`, `3(4+5)`, `2i`) | 1, 3, 4 | **in-model** — required for `a+bi` literals | parser |
| Trig + inverse + hyperbolic + inverse hyperbolic | all 5 | **in-model** | 18 functions |
| `sec` / `csc` / `cot` | 3 | **in-model** (cheap reciprocals) | function table |
| `ln`, `log` (base-10), `log2`, `log(x, b)` | all 5 | **in-model** | function table |
| `sqrt`, `cbrt`, `root(x, n)` | 1, 2, 4, 5 | **in-model** | function table |
| `abs`, `sign`, `floor`, `ceil`, `round`, `trunc` | 1, 2, 5 | **in-model** | function table |
| `gcd`, `lcm` | 1, 2, 5 | **in-model** | function table |
| `ncr` / `npr` (combinations, permutations) | 1, 5 | **in-model** | function table |
| `hypot`, `atan2`, `min`, `max`, `pow` | 1, 5 | **in-model** | function table |
| Constants `pi`, `e` | all 5 | **in-model** | `pi`/`π`, `e`, plus `tau` |
| Constant `phi` (golden ratio) | 2 | **in-model** | `phi` |
| Imaginary unit `i` | 1, 3, 4, 5 | **in-model** | `i` constant (double engine) |
| Angle mode deg / rad | 1, 3, 4 | **in-model** | `angle_unit` |
| Angle mode gradians | 3 | **in-model** | `angle_unit = gradians` |
| Complex arithmetic + complex-valued functions | 1, 3, 4, 5 | **in-model** | `Complex64` engine |
| Complex output in polar/phasor form | 3 | **in-model** | `complex_form = polar` |
| `re` / `im` / `arg` / `conj` accessors | 3 | **in-model** | function table |
| Selectable significant digits | 1, 2, 4 | **in-model** | `precision` 1–200 |
| Arbitrary-precision arithmetic beyond double | 1, 2, 4 | **in-model** | `astro-float` engine at `precision ≥ 16` |
| Named variables / assignment | 1, 2, 4 | **in-model** | `name = expr` lines + `variables` param |
| `ans` (previous result) | 1, 5 | **in-model** | `ans` identifier |
| Multi-line / worksheet evaluation | 2 | **in-model** | one expression per line, `;` also splits |
| Notation: auto / fixed-decimal / scientific / engineering | 2, 4, 5 | **in-model** | `notation` |
| Digit grouping (thousands separators) | 2 | **in-model** | `group_digits` |
| Machine-readable export | — (gizza convention) | **in-model** | `output_format = json` |

## Out-of-model (listed, not built)

| Feature | Seen at | Why it is out of model here |
|---|---|---|
| Graphing / function plotting | 1, 4 | Interactive plot surface; gizza tool pages are one-shot input→output |
| Matrix and linear-algebra mode | 1 | Separate value domain and UI; belongs in its own tool |
| Statistics mode, unit-conversion mode | 1 | Already covered by `blocks/descriptive-stats` and `blocks/unit-converter` |
| Symbolic derivative / integral / equation solving / limits | 4 | Needs a CAS; no wasm-safe pure-Rust CAS in the toolkit |
| Gamma / digamma / Riemann zeta | 3 | Niche special functions; no wasm-proven arbitrary-precision implementation available |
| Exact rational / fraction display of results | 2, 5 | `blocks/decimal-to-fraction` already converts a decimal to a fraction |
| Primality testing (Miller–Rabin) | 2 | Different tool shape (predicate, not evaluation); `blocks/prime-*` territory |
| Memory keys MS/MR/M+/M−/MC, calculation history | 1, 3, 4 | Stateful UI session; gizza tools are stateless per run (`variables` + `ans` cover the need) |
| Code export (LaTeX / Python / JS / C++) | 1 | Transpilation, not evaluation; `blocks/latex-math-to-svg` covers the LaTeX direction |
| Auto-updating dependent-variable workspace with circular-dependency detection | 2 | Spreadsheet-style reactive model; lines here evaluate top-to-bottom once |
| Complex results at `precision ≥ 16` | — | `astro-float` has no complex type; the extended-precision engine is real-only and says so |

## Deliberate behaviour choices

- **`precision ≤ 15` → double/complex engine; `precision ≥ 16` → extended-precision real engine.**
  Both advertised capabilities ship, but not simultaneously; the error message names the fix.
- **`%` means modulo, not percent.** Competitors split on this; modulo is the unambiguous reading
  next to `mod(a, b)` and is documented on the page.
- **No silent cleanup of floating-point residue.** `sin(pi)` reports `1.22464679914735e-16`
  rather than a faked `0`; an FAQ explains why and points at `round()` / higher `precision`.
- **Polar form applies only to values with a non-zero imaginary part**, so `2 + 2` never renders
  as `4 ∠ 0°`.
