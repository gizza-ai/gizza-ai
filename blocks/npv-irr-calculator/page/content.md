## About this tool

Use this calculator to check the economics of a project, acquisition, equipment purchase, subscription stream, or any other cash-flow series. Paste the cash flows directly from a spreadsheet, or keep the upfront cost separate in `initial_investment`. The report gives the net present value at your required annual return, the internal rate of return, modified IRR, profitability index, plain payback, discounted payback, and a per-period table.

The parser accepts common finance worksheet formats: currency symbols, thousands separators, accounting negatives such as `(1,234)`, and a repeat shorthand such as `12x2500` for twelve equal future cash flows. Choose annual, semiannual, quarterly, monthly, or weekly periods so the annual discount rate is converted to the right per-period rate and IRR is annualized consistently.

### Worked example

For a project that costs $100,000 today and returns $30,000 at the end of each of the next five years, use:

```
cash_flows=-100000, 30000, 30000, 30000, 30000, 30000
discount_rate=8
period=annual
timing=end
```

The summary reports an NPV of about `$19,781.00`, an IRR of about `15.24% per year`, MIRR, profitability index, and the discounted table. If your cash-flow column already starts after the upfront cost, put `100000` in `initial_investment` instead and enter only the five positive flows.

### Limits and edge cases

- Up to **1,200 cash-flow periods** per run, including period 0.
- `discount_rate` is an annual percentage. Enter `10` for 10%, not `0.10`.
- `initial_investment` is entered as a positive cost and is inserted as a negative period-0 cash flow.
- Cash flows must include at least two periods. A series with no sign change has no meaningful IRR, and the tool says so instead of inventing one.
- When cash flows change sign more than once, several IRRs may exist. Use the NPV at your required rate or MIRR as the steadier decision metric.
- The output is educational arithmetic only and is not financial, investment, tax, or accounting advice.

## FAQ

<details>
<summary>What is the difference between NPV and IRR?</summary>

NPV discounts every cash flow at your required annual return and reports the value in money units. IRR solves for the rate that would make NPV equal zero. NPV is usually better for comparing projects of different size, while IRR is useful as a break-even return percentage.

</details>

<details>
<summary>Should I put the first cash flow in the series or in initial investment?</summary>

Either works as long as you do not count it twice. If your pasted series starts with the period-0 outflow, leave `initial_investment=0`. If your spreadsheet column starts with future inflows, enter the upfront cost as a positive number in `initial_investment` and paste only the later flows.

</details>

<details>
<summary>How does monthly or weekly timing change the answer?</summary>

The discount rate is always entered as an annual percentage. For monthly cash flows, a 12% annual rate becomes 1% per month; for weekly cash flows it is divided by 52. IRR is solved per period and then annualized with the same period count.

</details>

<details>
<summary>Why does the report warn about multiple sign changes?</summary>

A cash-flow series like negative, positive, negative can cross zero at more than one discount rate, so a single IRR may be misleading. The report counts sign changes and still shows NPV, MIRR, and payback so you can use metrics that remain well-defined.

</details>

<details>
<summary>Can I export the discounted table?</summary>

Yes. Set `output=csv` for a comma-separated table with period, cash flow, discount factor, present value, cumulative present value, and cumulative undiscounted cash flow. Set `output=json` when you need the full analysis object for another script.

</details>
