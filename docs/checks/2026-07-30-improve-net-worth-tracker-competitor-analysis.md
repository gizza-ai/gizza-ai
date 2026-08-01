# net-worth-tracker — competitor analysis (2026-07-30)

Scan of the top real "net worth calculator / tracker" tools. All findings are paraphrased;
no competitor copy, branding, or trademarks were reused. Ideas/features/UX patterns only.

## Competitors surveyed

1. **NerdWallet — Net Worth Calculator** (nerdwallet.com/investing/calculators/net-worth-calculator)
   Manual entry, no linking. Suggests asset groups (cash/savings, retirement 401(k)/IRA, home
   value, stocks/bonds, vehicles/collectibles, land) and liability groups (mortgages, car loans,
   credit cards, medical, student, business, personal). Output: single net-worth figure with
   component breakdown. Strong framing: "wealth, not income" — a high earner can still be negative.

2. **Bankrate — Personal Net Worth Calculator** (bankrate.com/personal-finance/personal-net-worth-calculator)
   Manual entry across standard asset/liability groups. Differentiator: a **10-year projection** of
   how net worth could grow — turns the snapshot into a forward-looking view. Requires persistence.

3. **Empower (formerly Personal Capital) — Net Worth Tool** (empower.com/tools/net-worth)
   Account **aggregation/linking** (banks, cards, brokerage, retirement, crypto, real estate via
   Zillow, vehicles). Live figure, composition charts, longitudinal tracking. Requires an account.

4. **Vertex42 — Net Worth Calculator (Excel/Sheets)** (vertex42.com/Calculators/net-worth.html)
   Downloadable spreadsheet, line-by-line entry. Rich, fully-editable category rows (CDs, life-
   insurance cash value, bullion, notes receivable, personal property). Offline/local privacy
   appeal — the closest analog to a browser-local paste tool. Net worth = assets − liabilities with
   category subtotals + a simple chart.

5. **Kubera — Net Worth Tracker (premium)** (kubera.com)
   Connected accounts, tickers, crypto/DeFi wallets, physical assets with AI appraisal. First-class
   **multi-currency**, historical performance, allocation views, cohort benchmarking. Server/account.

**Bonus reference — miniwebtool Net Worth Calculator:** the fullest pure-compute UX — real-time
totals, an asset-vs-liability balance bar, **per-category percentage bars**, a debt-to-asset
"financial health" signal, custom categories, and scenario presets.

## Table-stakes features

1. Two itemized sides (assets + liabilities) with running subtotals.
2. Core output: **Net worth = Total Assets − Total Liabilities**.
3. Recognizable preset categories, extensible with custom labels.
4. Per-category breakdown with each category's share of its side.
5. A visual (bar/percent) making composition legible at a glance.
6. A derived health ratio — debt-to-asset.
7. A clear definition + light education ("wealth, not income").

## Gap analysis vs our tool

| Feature | Status in net-worth-tracker |
| --- | --- |
| Assets + liabilities two-sided totals | **Shipped** — `render_side` for each side. |
| Net worth = assets − liabilities | **Shipped** — headline line, handles negative net worth. |
| Per-category value + percent-of-side + bar + count | **Shipped** — `CategoryLine` + Unicode bars. |
| Debt-to-asset ratio | **Shipped** — plus equity % ("you own X% of your assets"). |
| Preset/recognizable categories | **Shipped as guidance** — page lists standard asset/liability categories; any label accepted. |
| Type column optional; sign-infers liabilities | **Shipped** — negative / `(…)` amounts = liability; explicit `asset`/`liability` token wins. |
| Currency symbol/prefix | **Shipped** — `currency` param. |
| Sort largest-first / alphabetical | **Shipped** — `sort` param. |
| Currency amounts, thousands, accounting negatives, shares@price | **Shipped** — shared money parser. |
| Scenario / worked example presets | **Shipped** — two `[[example]]` chips. |
| Privacy / no-account positioning | **Shipped** — page copy + local-wasm framing. |

## Considered, out of model (not built — need server/account/live data)

- **Account aggregation / bank linking** (Empower, Kubera) — needs a backend + credentials.
- **Live prices / home valuation / AI appraisal** (Kubera) — needs live data feeds.
- **Over-time tracking + multi-year projection** (Bankrate 10-year, Empower trend) — needs
  persistence across sessions; a stateless paste tool has no history to trend.
- **FX multi-currency conversion** (Kubera) — the `currency` field is a passive label/prefix only;
  converting across currencies would need live/entered rates. A user can pre-convert their figures.

## SEO angle (original copy)

Question-form + definitional title ("Net Worth Calculator — Total Assets Minus Liabilities"),
emphasizing the differentiators competitors lack: no account, no sign-up, runs in the browser,
paste a list. FAQ answers the common user queries: how net worth is calculated, asset vs
liability, what the debt-to-asset ratio means, is it advice, is my data private, what the limits
are.
