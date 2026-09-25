## About this tool

This calculator turns one reporting period of ad spend and attributed revenue into the whole return-on-ad-spend picture: gross ROAS, ROAS as a percentage, ACOS, net ROAS after margin, the break-even ROAS your margin implies, a target ROAS for the net margin you want, and an LTV-adjusted ROAS when customers buy more than once.

The number everything hinges on is the contribution margin, so there are two ways to set it. **Single gross margin percentage** takes one figure and computes `break-even ROAS = 1 ÷ gross margin`. **Build margin from per-order costs** derives it from what an average order really costs you — cost of goods, shipping and fulfilment, payment processing as a rate plus a fixed fee, a refund allowance and any other variable cost — which is what an ecommerce break-even actually depends on.

Fill in the order count and you also get CAC, the most you can afford to pay for an order at break-even and at your target margin, LTV, the LTV:CAC ratio and how long CAC takes to pay back. Fill in clicks or a cost per click and you get CPC, clicks per order, revenue per click and the break-even CPC. Set a revenue goal and you get the budget it needs at each ROAS. Everything runs locally in your browser — no numbers are uploaded.

### Worked example

Ad spend of `10000`, revenue from ads of `35000` and a `50%` gross margin give:

```text
ROAS                                            3.50x
Break-even ROAS                                 2.00x
Net ROAS (margin per unit spent)                1.75x
ACOS (spend ÷ revenue)                          28.57%
Margin on that revenue                          $17,500.00
Profit after ad spend                           $7,500.00
Break-even ad spend (most you could spend)      $17,500.00
Break-even revenue (least this spend must return) $20,000.00
Headroom above break-even ROAS                  75.00%
```

The campaign returns `3.50` of revenue per unit spent against a `2.00` break-even, so there is `75%` of headroom before it stops paying for itself — and at this revenue you could have spent up to `$17,500` and still broken even.

### Formulas used

- `ROAS = revenue ÷ ad spend`
- `ACOS = ad spend ÷ revenue`
- `Break-even ROAS = 1 ÷ contribution margin`
- `Target ROAS = 1 ÷ (contribution margin − target net margin)`
- `Contribution margin per order = AOV − cost of goods − shipping − (payment rate × AOV) − fixed fee − refund allowance − other variable cost`
- `CAC = ad spend ÷ orders`, `LTV = AOV × lifetime purchases`, `LTV:CAC` compares lifetime **margin** to CAC
- `Forecast: clicks = ad spend ÷ CPC`, `orders = clicks × conversion rate`, `revenue = orders × AOV`

### Limits and assumptions

- Every input must cover the **same** reporting period. The calculator never assumes a day, a month or a year — the outputs inherit whatever period you put in.
- Revenue is whatever your ad platform or analytics attributes to the spend. This tool performs no attribution modelling of its own, and cannot tell incremental revenue from revenue you would have earned anyway.
- The refund allowance is deducted as `refund rate × order value`, i.e. a refunded order is treated as returning no revenue.
- `LTV:CAC` is computed on lifetime **contribution margin**, not lifetime revenue, so a healthy ratio here is stricter than a revenue-based one.
- The currency field is a display prefix only. No exchange rates are fetched and no live ad-platform data is read.
- CSV output is a `section,item,value` block followed by the what-if table as its own CSV block, separated by a blank line.
- Planning arithmetic for marketers, not financial, tax or investment advice.

## FAQ

<details>
<summary>What is a good ROAS?</summary>

There is no universal number — it depends entirely on your margin. Break-even ROAS is `1 ÷ contribution margin`, so a business keeping `50%` of each sale breaks even at `2.00x`, while one keeping `20%` needs `5.00x` just to stand still. Enter your margin and read the break-even figure; anything above it is profit on the ad-driven revenue, anything below it is a loss.

</details>

<details>
<summary>What is the difference between ROAS and ACOS?</summary>

They are the same relationship inverted. ROAS is revenue divided by ad spend, shown as a multiple like `3.50x`. ACOS is ad spend divided by revenue, shown as a percentage — `28.57%` in the example above. A rising ROAS is a falling ACOS. Both are reported here so you can match whichever your ad platform shows.

</details>

<details>
<summary>Should I use the gross margin percentage or the per-order costs?</summary>

Use the percentage when you already know your blended margin and want a fast answer. Switch **Margin model** to **Build margin from per-order costs** when you sell physical goods: shipping, payment processing and returns can move the real contribution margin several points away from the headline gross margin, and the break-even ROAS moves with it.

</details>

<details>
<summary>Can I use this before a campaign has run?</summary>

Yes. Leave revenue and orders empty and supply an average order value, a conversion rate and a cost per click. The calculator derives clicks from `ad spend ÷ CPC`, orders from `clicks × conversion rate` and revenue from `orders × AOV`, then reports the same metrics as a forecast. The **Forecast a campaign before launch** example chip fills this in for you.

</details>

<details>
<summary>How is the LTV-adjusted ROAS calculated, and when should I trust it?</summary>

It is `revenue × lifetime purchases per customer × contribution margin ÷ ad spend`. It answers "what does this spend return once customers buy again?" rather than "what did it return today". It is only as good as your repeat-purchase estimate, so treat it as a planning upper bound — cash flow still runs on the first-order ROAS, which is why the CAC payback figure is reported alongside it.

</details>

<details>
<summary>Why is my target ROAS rejected?</summary>

A target net margin has to stay below the contribution margin. If orders leave `20%` of margin, no amount of ad spend can leave a `30%` net margin on that revenue — the target is unreachable and the calculator says so rather than returning a misleading number. Lower the target, or raise the margin.

</details>
