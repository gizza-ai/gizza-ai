# chi-square-test — competitor analysis (2026-06-22)

Pearson's chi-square test tool. Two modes: goodness-of-fit and contingency
(test of independence). Three surfaces verified: chat/LLM schema + manifest,
`gizza tool chi-square-test`, and the `/tools/chi-square-test/` page.

## Surface check (Phase 1)

- **Chat / manifest:** descriptor `to_schema_json()` == hand-kept `manifest.json`
  `tool.parameters`; drift-guard unit test passes. Params: `mode`
  (enum, default goodness-of-fit), `observed` (required), `expected` (optional),
  `yates` (boolean, default false).
- **CLI:** goodness-of-fit (uniform + ratios), contingency, and Yates all return
  correct structured JSON (chi_square, degrees_of_freedom, p_value, expected,
  cramers_v, low_expected_warning, yates_correction).
- **Page:** 4 Playwright tests green — gof statistic/df/p-value, gof with expected
  ratios, contingency + Cramér's V, and 2×2 with Yates correction. `mode` renders
  as a `<select>`, `yates` as a checkbox.

## Competitors surveyed (top 5)

1. **GraphPad QuickCalcs** — contingency table calculator; offers Yates'
   continuity correction for 2×2; also Fisher's exact for small samples.
2. **GigaCalculator** — both goodness-of-fit and independence; arbitrary N×M
   tables; reports statistic, df, p-value.
3. **QuantPsy (Preacher)** — interactive gof + independence; Yates' correction
   for 1-df tests.
4. **StatMate** — independence + gof; reports χ², p-value, **Cramér's V**, and
   APA-formatted result string.
5. **Statistics LibreTexts / MyTimeCalculator** — observed vs expected entry;
   χ² statistic, p-value, and **critical value** at a chosen α.

## Gap analysis (fit-to-model)

| Competitor feature | Status in gizza tool |
|---|---|
| Goodness-of-fit (uniform + custom expected) | ✅ present (expected counts or ratios, auto-rescaled) |
| Contingency / independence (N×M) | ✅ present (arbitrary r×c) |
| Chi-square statistic + df + p-value | ✅ present (p-value via regularized incomplete gamma, no deps) |
| **Yates' continuity correction (2×2)** | ✅ **added this pass** (`yates` flag; auto-restricted to 2×2) |
| Cramér's V effect size | ✅ present (contingency) |
| Low-expected-count (<5) assumption warning | ✅ present (`low_expected_warning` + cell count) |
| Expected-count reporting | ✅ present (per-cell `expected` array) |
| APA-formatted result string | partial — page emits a plain-English reject/fail-to-reject line; no APA template (low value, skipped) |
| Critical value at α | not added — p-value already gives the reject/accept decision at any α; a fixed-α critical value is redundant and would add UI noise (skipped) |
| Fisher's exact test (small samples) | **out of scope** — a different test, not a chi-square variant; would be its own tool |

## Improvements applied this pass

- Added **Yates' continuity correction** for 2×2 contingency tables (`yates`
  boolean, default off; silently ignored for non-2×2 tables, surfaced via the
  `yates_correction` output flag and a page line). Verified against R's
  `chisq.test(..., correct=TRUE)` (X² ≈ 0.4464 for the [[10,20],[30,40]] case).
- Plumbed the flag through all three surfaces (chat arg, CLI, page checkbox) and
  added unit + Playwright coverage.

## Out-of-model / deliberately skipped

- Fisher's exact test (distinct test → separate tool).
- APA result-string templating and fixed-α critical-value table (low marginal
  value; the p-value already supports any significance threshold).

No competitor copy, branding, or trademarks were used.
