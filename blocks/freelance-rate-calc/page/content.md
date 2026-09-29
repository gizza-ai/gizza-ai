## About this tool

Use this calculator to turn an annual freelance income goal into the rates you need to quote. It starts with the take-home amount you want, adds annual business costs, health insurance and retirement contributions, grosses up for tax, then divides the required revenue by the billable hours that remain after vacation, holidays, sick days and non-billable work.

The result includes an hourly rate, a day rate, a billable-week rate, an average monthly rate, the effective rate per worked hour, and a breakdown of the assumptions. Optional fields let you compare a rate you already charge, add a slow-month buffer, or price a fixed project with a simple/standard/complex/rush multiplier and a 30/40/30 milestone split.

### Worked example

For a target take-home income of `$80,000`, `$3,000` of business expenses, a `30%` effective tax rate, `4` vacation weeks, `10` holidays, `5` sick days and `70%` billable time, the default report shows:

```text
Hourly rate: $93.08
Day rate (8-hour day): $744.67
Billable hours: 1,260.0
Revenue you must invoice: $117,285.71
```

Set **Tax model** to **Add US self-employment tax layer** when you want the calculator to add the flat 15.3% US self-employment tax assessed on 92.35% of net profit. Otherwise, keep it at **Income tax only** and include all tax effects in the effective tax-rate field.

### Limits and assumptions

- This is a planning calculator, not tax advice. It does not include progressive brackets, deductions, VAT/GST, state-specific rules or market-rate datasets.
- The tax rate is an effective percentage on profit. Costs are added as annual costs and the take-home target is grossed up correctly; it is not a naive `net × (1 + rate)` markup.
- Currency is a display prefix only. No exchange-rate lookup is performed.
- Billable percent is applied after time off is removed from the working calendar.

## FAQ

<details>
<summary>Why does the tax math divide by the amount I keep?</summary>

If you need `$80,000` after a `30%` tax, charging `$80,000 × 1.30` would leave only `$72,800` after tax. The correct gross-up is `$80,000 / (1 - 0.30)`, because the tax is assessed on the gross profit you earn.

</details>

<details>
<summary>What should I enter for billable percent?</summary>

Use the share of worked time that you can actually invoice. Many freelancers spend time on sales, admin, bookkeeping, learning, proposals and gaps between projects. If you work 40 hours but can bill about 28 of them, enter `70`.

</details>

<details>
<summary>How do I model health insurance or retirement contributions?</summary>

Enter them as annual amounts in the dedicated fields. They are added to the revenue the business must invoice, separate from your personal take-home target, so the rate covers them explicitly.

</details>

<details>
<summary>Can I use this outside the United States?</summary>

Yes. Leave **Tax model** at **Income tax only** and enter your own effective tax percentage. The optional self-employment layer is a US-specific shortcut; the rest of the calculator is just arithmetic over your currency, time and costs.

</details>
