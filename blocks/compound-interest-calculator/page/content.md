## About this tool

This compound interest calculator shows how money grows when interest is added back to the
balance and then earns interest of its own. Enter your **initial principal**, the **annual
interest rate**, and a **term** in years (plus optional extra months), choose how often
interest **compounds**, and — if you save regularly — add a **contribution** with its own
frequency and timing. Everything runs locally in your browser; no numbers are uploaded.

It returns the **future value**, the **total you contributed**, the **total interest earned**,
the **effective annual rate** (APY implied by your nominal rate and compounding), the number of
years for the principal alone to **double**, and a **year-by-year schedule** of balance,
contributions-to-date and interest-to-date.

Under the hood every combination of frequencies is reduced to one effective annual rate
(`EAR = (1 + r/n)^n − 1`, or `e^r − 1` for continuous compounding), the principal grows by
`(1 + EAR)^t`, and regular deposits are valued as an annuity at the equivalent
contribution-period rate — with a start-of-period (annuity-due) option when you deposit at the
beginning of each period. A negative contribution models a regular withdrawal.

### Example

Enter principal **1000**, rate **5**, **10** years, compounding **monthly**, contribution
**0**:

```json
{
  "future_value": 1647.01,
  "principal": 1000.0,
  "total_contributions": 0.0,
  "total_interest": 647.01,
  "effective_annual_rate": 5.1162,
  "years_to_double": 13.892,
  "years": 10.0,
  "schedule": [ ... ],
  "summary": "1,000.00 at 5% compounded monthly over 10 years grows to 1,647.01 (0.00 contributed, 647.01 interest)"
}
```

Compounding monthly instead of annually lifts the effective rate from 5% to **5.1162%**, so the
same nominal 5% ends at **1,647.01** rather than 1,628.89. Add a **500/month** contribution and
the future value climbs past **79,000** as the annuity stacks on top of the growing principal.

### Limits & notes

- Every field is optional and falls back to a sensible default (principal **1000**, rate **5%**,
  term **10 years**, **monthly** compounding, **no** contributions, deposits at **end** of
  period).
- The rate is a **nominal** annual percent; the tool reports the **effective** annual rate
  (APY) separately so you can compare accounts quoted either way.
- Term is capped at **1000 years** and the schedule is capped at **121 rows** (the final term
  boundary is always included).
- Money is rounded to **2 decimals** and rates to **4**; figures are estimates for planning and
  your institution's exact rounding, fees and day-count may differ.
- Enter amounts as plain numbers — no currency symbols or thousands separators.

## FAQ

<!-- FAQ entries are <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What's the difference between the nominal rate and the effective annual rate (APY)?</summary>

The **nominal** rate is the headline annual percentage before compounding — the `annual_rate`
you type. The **effective annual rate** (APY, returned as `effective_annual_rate`) is what you
actually earn once interest compounds during the year. At 5% nominal compounded monthly the
effective rate is about **5.1162%**, because each month's interest itself earns interest for the
rest of the year. The more often it compounds, the larger the gap.

</details>

<details>
<summary>How do regular contributions work, and what does "timing" mean?</summary>

Set a `contribution` amount and a `contribution_frequency` (weekly, bi-weekly, monthly,
quarterly, semi-annually or annually) and each deposit is valued as an annuity that grows with
the account. **Timing** chooses when in each period the deposit lands: **end of period**
(ordinary annuity, the default) or **start of period** (annuity-due). Depositing at the start
gives every contribution one extra period of growth, so it always ends slightly higher. A
**negative** contribution models a regular withdrawal.

</details>

<details>
<summary>What does "years to double" tell me?</summary>

`years_to_double` is how long the **initial principal alone** would take to double at the
effective annual rate, ignoring contributions — a quick sense of the rate's power. At an
effective 5.12% it's about **13.9 years**, close to the rule-of-72 estimate (72 ÷ 5 ≈ 14.4).
It's `null` when the rate is zero or negative, since the balance never doubles from interest.

</details>

<details>
<summary>Which compounding frequency should I choose?</summary>

Use whatever your account states. Savings accounts and many loans compound **monthly** or
**daily**; bonds often **semi-annually**; some quote a simple **annual** figure. **Continuous**
compounding (`e^r`) is the theoretical upper bound and useful for modelling. The tool converts
any choice to a single effective rate, so you can change it and instantly compare the effect on
your future value.

</details>

<details>
<summary>Is my financial data sent anywhere?</summary>

No. The calculation runs entirely in your browser via WebAssembly. Nothing you enter is
uploaded, logged or stored — reload the page and it's gone.

</details>
