# financial-ratio-analyzer — competitor analysis (2026-09-04)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased observations of publicly visible behaviour. **No competitor copy,
branding, wording, or trademarks were reused** — only the set of line items and standard
textbook ratio formulas, which are common accounting knowledge.

## Tools reviewed

| # | Tool | Reachable | Shape |
|---|------|-----------|-------|
| 1 | Calkoo — Financial Ratio Analysis | yes | ~17 numeric fields, dashboard of ~14 ratios in 3 groups |
| 2 | CreditGuru — Financial Ratio Analysis Mega Calculator | yes | full statement grid (~30 line items), ~18 ratios + Altman Z-Score |
| 3 | UltimateFinanceCalculator — Financial Ratios Calculator | yes | 10 fields, 10 ratios, benchmark ranges + 0–100 health score |
| — | Ryan O'Connell CFA — Financial Ratio Calculator | **403** | replaced by #3 (per skill rule: replace an unreachable competitor) |
| — | StatementExtract — Financial Ratio Calculator | **403** | replaced by #3; search snippet advertises "20+ ratios with industry benchmarks" |

## Table-stakes observed, and where each landed

### Input line items (union across #1–#3)

| Line item | Seen in | Our handling |
|---|---|---|
| Cash & equivalents | 1, 2 | recognized label `cash` |
| Marketable securities / short-term investments | 1, 2 | `marketable_securities` |
| Accounts receivable | 1, 2 | `accounts_receivable` |
| Inventory | 1, 2 | `inventory` |
| Prepaid expenses | 2 | `prepaid_expenses` |
| Total current assets | 1, 2, 3 | `current_assets`, **derived** from the components when omitted |
| Fixed / non-current assets | 1, 2 | `fixed_assets`, derived as `total_assets - current_assets` |
| Total assets | 1, 2, 3 | `total_assets`, derived from `current + fixed` |
| Accounts payable | 2 | `accounts_payable` |
| Notes payable / current portion of LTD | 2 | `short_term_debt` |
| Total current liabilities | 1, 2, 3 | `current_liabilities`, derived from components |
| Long-term liabilities / debt | 2 | `long_term_debt` |
| Total liabilities | 1, 2, 3 | `total_liabilities`, derived |
| Retained earnings | 2 | `retained_earnings` (feeds Altman Z) |
| Total equity | 1, 2, 3 | `total_equity`, derived as `total_assets - total_liabilities` |
| Net sales / revenue | 1, 2, 3 | `revenue` |
| COGS | 1, 2, 3 | `cogs` |
| Gross profit | 1, 2 | `gross_profit`, derived as `revenue - cogs` |
| Operating expenses | 2, 3 | `operating_expenses` |
| Operating income / EBIT | 1, 2, 3 | `operating_income` / `ebit`, derived from `gross_profit - opex` |
| EBITDA | 2 | `ebitda`, derived as `operating_income + depreciation_amortization` |
| Interest expense | 1, 2, 3 | `interest_expense` |
| Income tax | 2 | `taxes` (also gives the effective tax rate for ROIC) |
| Net income / net profit | 1, 2, 3 | `net_income` |
| Market value of equity | 2 (Altman Z) | `shares_outstanding` × `share_price`, else book equity → Z′ variant |
| Period days selector (365/360/90/…) | 2 | `days_in_period` param, default 365 |

### Ratios

| Group | Competitor ratios | Ours |
|---|---|---|
| Liquidity | current, quick, cash, net working capital | all four, **+** working-capital-to-revenue |
| Leverage | D/E, debt ratio, equity ratio, equity multiplier | all four, **+** LT-debt-to-equity, net debt, net-debt-to-EBITDA |
| Coverage | times interest earned (EBIT and EBITDA basis) | both |
| Margins | gross, operating, net, EBITDA/operating-profit margin | all, **+** pretax margin |
| Returns | ROA, ROE | both, **+** ROCE, ROIC, and a DuPont ROE decomposition |
| Efficiency | asset turnover, fixed-asset turnover, inventory turnover, AR turnover, DSO, AP-to-sales, cash conversion cycle | all except AP-to-sales (replaced by the stronger payables turnover + DPO), **+** DIO, working-capital turnover |
| Solvency | Altman Z-Score (#2) | Z (public, market cap available) and Z′ (private, book equity), variant stated in the output |
| Market | EPS/P-E family (advertised by the two 403 tools) | EPS, P/E, earnings yield, book value per share, price-to-book, market cap |

### UX patterns

| Pattern | Seen in | Ours |
|---|---|---|
| Benchmark "healthy range" per ratio | 3 | `benchmarks` boolean (default on): `ok` / `low` / `high` flag against a stated rule-of-thumb range |
| 0–100 health score | 3 | health score = share of benchmarked ratios inside their range, with the count shown |
| Grouped dashboard, all ratios at once | 1, 2, 3 | `summary` output prints every group; `groups` param narrows to one family |
| Reset button / blank until entered | 1 | n/a — the page has a Reset control generically; missing inputs print `n/a` with the reason |
| Sample/worked data | 3 | three `[[example]]` preset chips + a worked example in `content.md` |
| Period-days selector | 2 | `days_in_period` (365 default, 360/90/30 all accepted) |
| Interactive per-ratio explainer links | 1 | out of scope (single-page tool, no cross-linking in this repo) |

## Gaps we close that competitors do not

1. **Paste-driven input.** Competitors are fixed numeric-field grids. Ours parses a pasted
   `label: value` block straight out of a spreadsheet or a filed statement, with alias matching
   (`net sales` ≡ `revenue` ≡ `turnover`), currency symbols, thousands separators, accounting
   parentheses for negatives, and `1.2m` / `340k` / `2bn` scale suffixes.
2. **Derivation of omitted subtotals.** Any of `current_assets`, `total_assets`, `gross_profit`,
   `operating_income`, `total_equity`, `total_liabilities`, `ebitda` is computed from its parts
   when not pasted, and the report says which figures were derived.
3. **Prior-period column.** `prior_figures` adds a second statement; every ratio gains a change
   column, and `basis = average` uses average balance-sheet figures for ROA/ROE/turnover/DSO —
   the textbook-correct denominator none of the three competitors offer.
4. **DuPont decomposition** of ROE into margin × asset turnover × equity multiplier.
5. **Machine-readable output** — `csv` and `json` shapes; none of the three offer export.
6. **Balance-sheet consistency check** — warns when `assets ≠ liabilities + equity`.

## Out of model / deliberately not built

- **Industry-specific benchmark databases** (#3 advertises per-industry ranges). Requires a
  licensed dataset; we ship only clearly labelled generic rules of thumb.
- **Multi-year trend charts / graphs.** The page output surface is text; two periods are
  supported numerically, charting is not.
- **Automatic extraction from an uploaded PDF/XBRL filing** (StatementExtract's core pitch).
  Out of scope for a pure block; the existing `pdf-extract-text` block covers the extraction
  half and its output can be pasted in here.
- **Per-ratio explainer pages** (#1's interactive links) — this repo renders one page per tool.

## Verification performed

- `cargo test --workspace` (core + descriptor drift-guard)
- `scripts/build-block-wasm.sh financial-ratio-analyzer`, `wasm-pack build … --target web`
- `python3 scripts/sync-tool-manifest.py financial-ratio-analyzer`, generator render
- `gizza tool financial-ratio-analyzer …` — exact-output case plus one run per enum choice,
  both non-default booleans, and the line-count cap boundary
- `npx playwright test tool-page-financial-ratio-analyzer.spec.ts` — real output assertions and
  a `?param=` deep link
- `python3 scripts/check-tool-hygiene.py financial-ratio-analyzer`

Educational arithmetic only — the tool states on the page that it is not financial, investment,
tax, or accounting advice.
