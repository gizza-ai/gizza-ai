# rent-vs-buy — competitor analysis (2026-07-30)

Scan done before implementing. Goal: match the table-stakes inputs, defaults, worked
example, and UX patterns of the leading rent-vs-buy calculators, without copying any
competitor copy, branding, or trademarks (everything below is paraphrased).

## Competitors skimmed (top real tools for "rent vs buy calculator")

1. **calculator.net — Rent vs. Buy Calculator** — the most parameter-rich. Buy side:
   home price, down payment, loan term, interest rate, PMI, property-tax rate, home
   insurance, maintenance/HOA fees, buying closing costs, home-value appreciation,
   selling cost/commission. Rent side: monthly rent, renters insurance, rent growth.
   Shared: after-tax investment return, income-tax rate, general inflation. Outputs a
   year-by-year cost comparison and a break-even point.
2. **NerdWallet — Rent vs Buy Calculator** — consumer-friendly, slider-driven. Core
   inputs: home price, down payment (%), mortgage rate, how long you'll stay, monthly
   rent, plus advanced toggles for property tax, home appreciation, rent increase, and
   investment return. Headline output: "buying is better if you stay N+ years"
   (break-even year) and a net-worth comparison.
3. **Zillow — Rent vs Buy Calculator** — home price, down payment, rate, term, monthly
   rent, and time horizon; models the down payment + monthly savings as an invested
   side-fund (opportunity cost) and reports the break-even year.

(Fidelity and Leader Bank cover the same ground with fewer knobs; used to confirm the
common defaults rather than add new params.)

## Methodology all three share (this is the model we implement)

The credible calculators do NOT just compare monthly rent to a monthly mortgage. They
run an **"invest the difference" net-worth race** over a chosen horizon:

- Buyer spends the down payment + closing costs up front; the renter invests that same
  cash (opportunity cost) at the investment-return rate.
- Each month, whichever party has the lower housing outflow invests the difference.
- At the horizon the buyer's wealth = appreciated home value − selling costs − remaining
  mortgage + their side-fund; the renter's wealth = their side-fund.
- **Break-even year** = the first year the buyer's net worth catches the renter's.
  Investment return is the decisive hidden variable (high return favours renting because
  the down payment compounds; low return favours buying).

## Table-stakes params — tagged in-model / out-of-model

| Param | Tag | Decision |
|---|---|---|
| home price | in-model | `home_price`, default 400000 |
| down payment % | in-model | `down_payment_percent`, default 20 |
| mortgage rate % | in-model | `mortgage_rate_percent`, default 6.5 |
| loan term (yrs) | in-model | `loan_term_years`, default 30 |
| monthly rent | in-model | `monthly_rent`, default 2000 |
| time horizon (yrs) | in-model | `years`, default 10 |
| home appreciation % | in-model | `home_appreciation_percent`, default 3 |
| rent growth % | in-model | `rent_growth_percent`, default 3 |
| investment return % | in-model | `investment_return_percent`, default 5 |
| property tax %/yr | in-model | `property_tax_percent`, default 1.1 |
| home insurance %/yr | in-model | `home_insurance_percent`, default 0.5 |
| maintenance %/yr | in-model | `maintenance_percent`, default 1 |
| HOA / mo | in-model | `hoa_monthly`, default 0 |
| buying closing % | in-model | `buying_closing_percent`, default 3 |
| selling cost % | in-model | `selling_cost_percent`, default 6 |
| currency symbol | in-model | `currency`, default $ |
| decimals | in-model | `decimals`, default 0 |
| PMI (< 20% down) | out-of-model | Listed only. Adds a lender-specific rate table
  and drop-off rules; not built to keep the model transparent. Users can fold it into
  the maintenance/insurance figures. |
| mortgage-interest / SALT tax deduction | out-of-model | Listed only. Needs the user's
  marginal bracket, filing status, and standard-deduction crossover — jurisdiction- and
  year-specific; excluded rather than approximated wrongly. |
| renters insurance | out-of-model | Small and usually a wash; folded conceptually into
  rent. Not a separate param. |
| general inflation (separate from the rate knobs) | out-of-model | The appreciation,
  rent-growth and investment-return rates are already entered as nominal, so a separate
  inflation deflator would double-count; excluded. |

## UX control patterns to match (competitors ship these)

- **Sliders** for the bounded percentage rates (down payment, rates, growth, returns) —
  implement as `kind = "slider"` on those fields.
- **Preset chips** — competitors expose scenario presets ("stay 5 years", "high returns
  favour renting"). Ship `[[example]]` chips: a 10-year default, a short 3-year stay
  (renting wins), a long 15-year stay (buying wins), and a high-investment-return
  scenario.
- Headline should be a **verdict + break-even year**, not just a table — our `summary`
  and `break_even_year` output cover this.

## Limits stated on the page (honesty)

Nominal rates, monthly compounding, flat percentage costs, no PMI, no tax deductions, no
local price data — a planning estimate, not financial advice.
