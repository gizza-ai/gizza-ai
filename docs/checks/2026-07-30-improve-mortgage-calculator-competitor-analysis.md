# mortgage-calculator competitor analysis — 2026-07-30

## Sources skimmed

- Consumer mortgage payment calculators for fixed-rate home loans.
- Real-estate affordability calculators that add tax, insurance, HOA, and extra payment fields.
- Amortization calculators that show total interest and payoff timing.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitor tools | Model fit | Decision |
| --- | --- | --- | --- |
| Home price and down payment | Universal | In-model | Added `home_price` and cash-amount `down_payment`; loan amount is derived. |
| Loan term and interest rate | Universal | In-model | Added `loan_years` and `annual_interest_rate_percent`; term/rate use sliders on the page. |
| Principal-and-interest payment | Universal | In-model | Reported as `monthly_principal_interest` using the standard amortizing-loan formula. |
| Property tax and homeowner's insurance | Common | In-model | Added annual tax/insurance inputs and monthly + total outputs. |
| HOA / condo dues | Common | In-model | Added `monthly_hoa` and total HOA over the payoff period. |
| Full monthly payment / PITI-style breakdown | Common | In-model | Reported as `monthly_payment` with tax/insurance/HOA components. |
| Total interest and total cost | Common | In-model | Reported as `total_interest` and `total_cost`. |
| Extra monthly principal payments | Common in richer calculators | In-model | Added `extra_monthly_payment`; amortization loop computes shorter payoff and lower interest. |
| Amortization table by month/year | Some tools | Out-of-model for first release | Not emitted to keep output concise and page-friendly; aggregate payoff months and totals cover the core decision. |
| PMI, closing costs, APR, ARM resets, tax deductions | Some tools | Out-of-model / jurisdiction-specific | Not built; documented as excluded because formulas vary by lender, location, and loan product. |
| Affordability / income qualification | Some tools | Out-of-model for this slug | Separate planning tool shape; this block answers payment and total-cost math for a supplied loan. |

## Implementation notes

The tool is a pure Rust fixed-rate mortgage calculator. It computes the scheduled principal-and-interest payment analytically, then runs a bounded monthly amortization loop so extra payments reduce `payoff_months` and `total_interest` accurately. Taxes and insurance are annual inputs divided by 12; HOA is monthly. All copy is generic and avoids lender or real-estate brand language.
