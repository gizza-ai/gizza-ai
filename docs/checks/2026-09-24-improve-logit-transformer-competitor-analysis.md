# logit-transformer — competitor analysis (2026-09-24)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of what each tool *does*; no competitor copy, branding, or trademark text is reused.

## Dup check (why this tool is not a duplicate)

| Existing block | What it does | Why it is not this tool |
| --- | --- | --- |
| `z-score-normalize` | z-score / min-max / max-abs / robust scaling of a number column | linear rescaling driven by the column's own mean/σ; no odds, no log-odds, no (0,1) domain |
| `absolute-value-transformer` | unary sign transforms (abs / sign / negate / force-negative) on a column | sign-only arithmetic; no probability↔log-odds mapping |
| `data-normalize`, `json-normalize`, `csv-*-normalizer` | shape/format normalisation of records, dates, quotes, whitespace | structural cleanup, not numeric transforms |
| `log-return-calculator` | `ln(Pₜ / Pₜ₋₁)` on a price series | log of a *ratio between consecutive rows*; not `ln(p/(1−p))` on each row |
| `svm-classifier` | fits an SVM (its kernel list includes a "sigmoid" kernel) | `tanh` kernel inside a classifier; exposes no logit/expit transform of a user column |

Confirmed by reading each block's `core/src/lib.rs` and descriptor. No block converts probabilities
to log-odds or back.

## Competitors reviewed

1. **MedCalc — LOGIT function + logit transformation table** — `logit(p) = ln(p/(1−p))`; accepts a
   single probability **or a whole matrix** of values and returns results of matching shape; the
   argument must lie strictly between 0 and 1; documents `ALOGIT` as the inverse; ships a
   proportion→log-odds lookup table and an optional graph with configurable axis ranges.
2. **RedCrab — logit calculator** — one probability strictly in (0, 1); a **decimal-places
   selector** (0/1/2/3/4/6); states the function is undefined at the boundaries; gives the
   arctanh identity and worked anchors: `logit(0.5) = 0`, `logit(0.75) ≈ 1.099` (3:1 odds),
   `logit(0.25) ≈ −1.099` (1:3 odds).
3. **AZCalculator — log odds / odds converter** — a **three-way** converter: type a probability
   (0–1), odds (≥0) **or** log-odds (any real) and it fills in the other two, using
   `odds = p/(1−p)`, `log-odds = ln(odds)`, `p = e^L/(1+e^L)`. Frames the use cases as logistic
   regression, risk, and betting; has a reset control.
4. **Sebastian Sauer — convert logit to probability** — the inverse direction as practitioners
   actually use it: `odds = exp(logit)` then `p = odds/(1+odds)`; anchors `logit 0 → p 0.5` and
   `logit 1 → p ≈ 0.73`; worked end-to-end example turning logistic-regression coefficients into a
   predicted probability (~44%).
5. **EasyCalculation / AAT Bioquest — logit + logistic-regression calculators** — single-value
   "logit evaluation" for a proportion (EasyCalculation), and a full logistic-regression fitter
   returning β coefficients, p-values, standard errors, log-likelihood, deviance and AIC (AAT).

## Table stakes → decisions

| Capability | Seen in | Verdict |
| --- | --- | --- |
| `logit(p) = ln(p/(1−p))` for a probability | 1, 2, 3, 5 | **in-model** — `direction = logit` (default) |
| Inverse logit / sigmoid / expit back to a probability | 1, 3, 4 | **in-model** — `direction = inverse` |
| Batch / matrix of values in one run, not one value at a time | 1 | **in-model** — paste a whole column; `MAX_VALUES = 20,000` |
| Odds shown alongside the probability and log-odds | 3 | **in-model** — `output = table` emits `input⇥odds⇥result`; `output = json` carries the same triple |
| Decimal-places selector | 2 | **in-model** — `decimals = auto\|0…8` |
| Domain enforcement: `p` strictly inside (0, 1) | 1, 2 | **in-model** — default `on_boundary = fail` names the offending value and says what was expected |
| A usable answer at `p = 0` / `p = 1` instead of a dead end | (gap in 1–5) | **in-model** — `on_boundary = clamp` (ε-clamp, `epsilon` default 1e-6, the standard smoothing fix), plus `skip`, `blank`, and `infinity` (emit ±Infinity) |
| Percentages as input (`90%`, risk stated in %) | 3 (risk framing) | **in-model** — a `%` suffix on any token is accepted and divided by 100; a bare `90` is rejected with a message telling you to append `%` or divide |
| Log base other than *e* (base-10 / base-2 log-odds) | 1 (log-function family) | **in-model** — `base = e\|2\|10`, applied consistently to both directions so round-trips are exact |
| Configurable input separator; paste a spreadsheet column | 1 | **in-model** — `separator = auto\|newline\|comma\|space\|semicolon\|tab\|pipe` |
| Configurable output separator | 1 | **in-model** — `output_separator`, defaults to mirroring the input |
| Worked anchors on the page (0.5 → 0, 0.75 → 1.0986, logit 1 → 0.731) | 2, 4 | **in-model** — `content.md` worked examples + FAQ |
| One-click presets | 2, 3 | **in-model** — `[[example]]` chips for logit, sigmoid, percent input, ε-clamp, and the odds table |
| Lookup/reference table of proportion → log-odds | 1 | **in-model (equivalent)** — paste the proportions you care about and pick `output = table`; a fixed printed table is page decoration, not a capability |
| Plot of the S-curve | 1, 4 | **out-of-model** — plotting belongs to `csv-chart-generator`; stated as a limit rather than half-built |
| Fitting a logistic regression (β, p-values, AIC, deviance) | 5 | **out-of-model here** — that is a model-fitting tool, not a transform; this block transforms values you already have (and `svm-classifier` / the regression family own fitting) |
| Confidence intervals / standard errors on the transformed scale | 5 | **out-of-model** — needs per-row `n` or SE columns, i.e. a different tool shape (meta-analysis input), not a value-in/value-out transform |
| Currency/thousands-separator cleanup of messy cells | — | **out-of-model** — owned by `numeric-string-sanitizer`; the FAQ points there |

## Notes

- Nothing above was copied verbatim; every descriptor `.describe()`, page heading, worked example,
  and FAQ answer is written fresh for this block.
- Out-of-model items are listed here and, where user-visible, stated as limits on the page; none are
  silently dropped.
- Round-trip exactness is treated as a correctness requirement, not a nicety: `inverse(logit(p)) = p`
  to within float precision for every supported base, and it is covered by a unit test.
