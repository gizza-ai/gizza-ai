## About this tool

Estimate a fixed-rate mortgage payment from the home price, down payment, loan
term, and interest rate. The calculator breaks the monthly payment into
principal and interest, property tax, insurance, and HOA dues, then totals the
interest and ownership costs over the payoff period.

Worked example: a `400000` home with an `80000` down payment leaves a `320000`
loan. At `6.5%` over `30` years, the principal-and-interest payment is about
`2022.62` per month before taxes, insurance, or HOA. Add annual tax,
homeowner's insurance, and HOA fields to estimate a fuller PITI-style monthly
housing payment.

Use `extra_monthly_payment` to model additional principal payments. The base
monthly payment stays the scheduled mortgage payment, while the amortization
loop applies the extra amount each month to calculate fewer payoff months and
lower total interest.

Limits and edge cases: this is a deterministic fixed-rate calculator, not loan
advice. It does not include PMI, closing costs, adjustable-rate resets, escrow
rules, tax deductions, late fees, biweekly schedules, or lender-specific
rounding. Enter taxes and insurance as annual amounts; enter HOA as a monthly
amount. The down payment is a currency amount, not a percent.

## FAQ

<details>
<summary>What does the monthly payment include?</summary>

`monthly_payment` includes scheduled principal and interest plus monthly taxes,
insurance, and HOA dues. It does not include the optional
`extra_monthly_payment`; that amount is applied separately to shorten the loan.

</details>

<details>
<summary>Is the down payment a percent?</summary>

No. Enter the down payment as a currency amount. For a 20% down payment on a
`400000` home, enter `80000`.

</details>

<details>
<summary>How are taxes and insurance handled?</summary>

Enter property tax and homeowner's insurance as annual amounts. The calculator
divides each by 12 and includes those monthly amounts in the payment and the
payoff-period totals.

</details>

<details>
<summary>How does an extra monthly payment affect the result?</summary>

The extra payment is applied directly to principal in the amortization loop.
That can reduce `payoff_months` and `total_interest`; the reported scheduled
principal-and-interest payment stays unchanged.

</details>
