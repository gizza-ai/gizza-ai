# npv-irr-calculator — competitor analysis (2026-09-04)

Scan run BEFORE implementing, per `/create-next-tool` step 4. One web search
("NPV IRR calculator online cash flow net present value internal rate of return"),
then the top three reachable, real competitor tools were skimmed. Everything below is
**paraphrased** — no competitor copy, branding, or assets were reused.

## Competitors reviewed

### 1. GigaCalculator — NPV calculator
- **Inputs:** initial investment (currency), discount rate (percent), investment term in
  years, and a year-by-year cash-flow grid.
- **Outputs:** net present value, internal rate of return, a gross return figure, and the
  undiscounted total of the cash flows.
- **Limits:** the per-year grid tops out at 25 periods.
- **UX:** plain number fields for the headline inputs plus an expanding per-year grid, one
  Calculate button, and a sidebar of related finance calculators.
- **Worked example:** a property held three years — purchase price, recurring annual costs,
  and a sale price — discounted at 5%, reported alongside the resulting IRR.
- **Not stated:** the discounting convention (end vs beginning of period) is never spelled out.

### 2. Calculator.net — IRR calculator
- **Inputs:** two separate modes. A *fixed recurring* mode (initial amount, holding period in
  years/months, ending balance, contribution frequency from annual down to weekly, and a
  beginning-vs-end-of-period timing switch) and an *irregular* mode (initial investment plus
  up to 50 annual cash flows).
- **Outputs:** the IRR percentage. There is no discount-rate input — the tool solves for the
  rate that zeroes NPV.
- **Copy:** two worked examples, including one that contrasts two projects with identical
  undiscounted totals but different timing to show why IRR differs.
- **Limits stated in copy:** IRR ignores project scale, and cash flows that change sign more
  than once can admit multiple IRRs.
- **UX:** a "show more input fields" expander for the cash-flow rows.

### 3. IQCalculators — NPV & IRR calculator
- **Inputs:** horizon in years, initial investment, a year-one cash flow with an annual growth
  rate, an asset-appreciation rate, and a discount rate.
- **Outputs:** NPV headline, annualized IRR, undiscounted total cash flow, terminal value, and
  a per-year "what if you sold here" NPV/IRR breakdown.
- **UX:** an editable results table with a fill-down control and a CSV download.
- **Limits stated:** sign changes can yield several IRRs or none; the tool reports the first
  root it finds.

## Table-stakes checklist and where each one landed

| Capability | Seen at | In/out of model | Decision |
| --- | --- | --- | --- |
| Arbitrary cash-flow series | all three | in-model | `cash_flows` free-text series, newline/comma/space separated |
| Separate "initial investment" field | all three | in-model | `initial_investment` (entered positive, becomes the period-0 outflow) |
| Discount rate in percent | Giga, IQ | in-model | `discount_rate`, default 10 |
| NPV headline | Giga, IQ | in-model | yes |
| IRR | all three | in-model | bracketed bisection, reported per period and annualized |
| Undiscounted total cash flow | Giga, IQ | in-model | yes (plus separate inflow/outflow totals) |
| Beginning vs end-of-period timing | Calculator.net | in-model | `timing` enum (`end` default, `begin`) |
| Non-annual period frequency | Calculator.net | in-model | `period` enum annual…weekly, drives IRR annualization |
| Fixed recurring cash flow entry | Calculator.net | in-model | `12x2500` repeat shorthand inside the series field |
| Per-period discounted cash-flow table | IQ | in-model | table with factor, PV, cumulative PV |
| CSV export of the table | IQ | in-model | `format = csv` (page also has a download link) |
| Multiple-IRR warning on sign changes | Calculator.net, IQ | in-model | sign-change count is reported and warned about |
| Period cap | Giga (25), Calculator.net (50) | in-model | 1,200 periods — far beyond both, stated on the page |
| Payback period | not shown by these three | in-model | added (plain and discounted) — a differentiator |
| Profitability index | not shown by these three | in-model | added |
| MIRR | mentioned by a BAII-style competitor in the result list | in-model | added, using the discount rate for both financing and reinvestment (documented) |
| Currency-formatted output | Giga, IQ | in-model | `currency` symbol param |
| Editable results grid / fill-down | IQ | out-of-model | a spreadsheet grid is a different product; the repeat shorthand covers bulk entry |
| Saveable/shareable scenario links | PropertyMetrics | partly in-model | the page's `?param=` deep links already share a whole scenario; no accounts, no server |
| Per-year "sell here" sensitivity grid | IQ | considered, rejected | doubles the output size for a niche real-estate framing; the cumulative-PV column already shows break-even |
| Ad-supported sidebar of related calculators | Giga | out-of-model | site-level concern, not a tool capability |

## Design conclusions carried into the descriptor

- The series is the primary input, and it must tolerate what people paste: currency symbols,
  thousands separators, accounting negatives, and blank lines.
- Because two competitors keep the initial investment separate from the series and one folds
  it in, the tool supports both: a non-zero `initial_investment` is inserted as the period-0
  outflow and the series then starts at period 1; leaving it at 0 means the series' own first
  value is period 0.
- IRR needs an honest failure mode. When every cash flow has the same sign there is no root,
  and the tool says so instead of printing a bogus rate.
- Every fixed-choice parameter is an enum so the page renders a `<select>`, and the output
  format selector covers summary / table / csv / json.
