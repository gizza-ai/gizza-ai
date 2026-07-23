## About this tool

This debt payoff planner turns a list of debts into a concrete plan to become debt-free.
Enter each debt on its own line as **name, balance, APR%, minimum payment**, choose a
payoff **method**, and add any **extra** you can pay each month. The planner simulates the
whole payoff month by month and returns the payoff order, your debt-free date, the total
interest you'll pay, and how much you save versus paying only the minimums.

It uses the standard **rollover** (a.k.a. cascade) method: your total monthly payment stays
constant, and whenever a debt is cleared its freed-up minimum rolls onto the next target
debt — so the payments accelerate as you go.

**Two methods:**

- **Snowball** — pay the **smallest balance** first. You clear whole debts quickly, which is
  motivating.
- **Avalanche** — pay the **highest APR** first. This costs the **least total interest**.

The result shows both side by side with a recommendation, so you can weigh quick wins against
lowest cost.

### Example

Enter these three debts, method **snowball**, extra **$300/month**, start **2026-01-01**:

```
Visa, 2500, 19.99, 75
Car Loan, 8000, 6.5, 200
Store Card, 600, 24.99, 25
```

You'll get a JSON plan whose payoff order starts with **Store Card** (the smallest balance),
then **Visa**, then **Car Loan**, along with each debt's payoff date and interest, the overall
`debt_free_date`, `total_interest`, a `minimum_only` baseline, and a `comparison` block with
`snowball` vs `avalanche` totals and the recommended method. Switch the method to **avalanche**
and the payoff order re-targets the highest-APR debt (Store Card at 24.99%, then Visa) for the
lowest total interest.

### Limits & notes

- Enter amounts as plain numbers — `$` and `%` are ignored, but **don't use thousands
  separators** (the comma splits the four fields, so write `2500`, not `2,500`).
- Up to **50 debts**; APR up to **100%**; the simulation is capped at **1200 months (100
  years)**.
- Interest is compounded **monthly** (APR ÷ 12) and rounded to the cent each month, so the
  figures are estimates for planning — your lender's exact rounding and fees may differ.
- If your budget can never out-run the interest, you get a clear error instead of a result.
- Everything runs locally in your browser. No amounts are uploaded and nothing is stored.

## FAQ

<!-- FAQ entries are <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What's the difference between the snowball and avalanche methods?</summary>

The **snowball** method pays your **smallest balance** first, so you clear individual debts
quickly and build momentum. The **avalanche** method pays the **highest APR** first, which
minimizes the **total interest** you pay. Both use the rollover method — a cleared debt's
payment cascades onto the next one. This planner runs both and tells you the interest and
time difference so you can choose.

</details>

<details>
<summary>How does the extra monthly payment work?</summary>

The extra amount is added on top of all your minimum payments every month and is applied to
the current **target** debt (the smallest balance for snowball, the highest APR for
avalanche). When that debt is paid off, its minimum plus the extra roll onto the next target,
so the payments snowball. Even a small extra can cut the payoff time and interest
significantly — try changing it to see the effect.

</details>

<details>
<summary>Why does it say my debts can never be paid off?</summary>

If the total of your minimum payments plus the extra is **less than the interest** that
accrues each month, the balances grow faster than you pay them down, so they'd never reach
zero. The planner detects this and returns an actionable error. Raise a minimum payment or add
an extra monthly payment until the budget covers at least the interest.

</details>

<details>
<summary>What is the "minimum-only" baseline?</summary>

It's what happens if you pay only each debt's minimum, with **no extra** and **no rollover**.
It's the slow, expensive path — the planner reports its debt-free date and total interest so
you can see exactly how many months and how many dollars your plan saves. If any minimum
doesn't cover its own interest, the baseline is flagged as never-ending.

</details>

<details>
<summary>Does this account for fees, promotions, or changing rates?</summary>

No. The planner assumes a **fixed APR** per debt, monthly compounding, and constant minimum
payments, and it ignores annual fees, late fees, and promotional/teaser rates. It's a planning
estimate — treat the numbers as close guidance, not an exact statement from your lender.

</details>
