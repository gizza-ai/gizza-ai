## About this tool

Turn a pasted survey column into the customer-experience score people expect to see in a status report. The calculator handles Net Promoter Score (NPS), Customer Satisfaction Score (CSAT), and Customer Effort Score (CES), then adds the pieces that make the number auditable: response counts, percentage bands, rating distribution, mean, standard deviation, and an optional confidence band.

You can paste raw ratings from a spreadsheet, comma-separated survey exports, or a pre-tallied `rating,count` table. Everything runs locally in the browser, so respondent-level data stays on your machine.

### Worked example

Paste this NPS column:

```
score
10
9
8
7
6
10
0
```

With `metric=nps`, the 10 and 9 ratings count as promoters, 8 and 7 are passives, and 6 plus 0 are detractors. The report shows the NPS score, the promoter/passive/detractor breakdown, the scale distribution, and a 95% confidence band. Switch `format` to `json` or `csv` when you want the same calculation in a pipeline.

For CSAT or CES, choose the metric, set the scale, and leave `threshold=-1` for the usual top-box default: 4+ on a 1–5 CSAT scale, 5+ on a 1–7 CES scale, or 9+ on a 0–10/1–10 scale.

### Limits and edge cases

- Up to **100,000 responses** per run. Use `input=counts` for large exports that are already summarized.
- NPS is fixed to the **0–10** scale; CSAT and CES can use 1–5, 1–7, 1–10, or 0–10 scales.
- Blank cells and `NA`, `N/A`, `-`, `.`, `none`, `null`, `missing`, and `?` are skipped and reported.
- Ratings must be whole numbers inside the selected scale. Out-of-scale values are errors, not clipped.
- The confidence band is a normal approximation. It is useful for quick comparisons, but very small samples should still be interpreted cautiously.
- Benchmark tiers vary by industry, region, product maturity, and survey channel. This tool reports a computed tier and the raw breakdown rather than embedding a stale benchmark table.

## FAQ

<details>
<summary>What is the difference between NPS, CSAT and CES?</summary>

NPS asks how likely someone is to recommend you and reports promoters minus detractors on a 0–10 scale. CSAT asks whether someone is satisfied and reports the share at or above your satisfied cut-off. CES asks how easy the experience was; this tool reports the mean effort score plus the share at or above the easy cut-off.

</details>

<details>
<summary>Can I paste an already summarized table?</summary>

Yes. Set `input=counts` and enter one `rating,count` row per line, such as `10,42` or `7: 12`. The calculator expands those counts mathematically, so the score and confidence band match what you would get from the raw response column without pasting every row.

</details>

<details>
<summary>Which threshold should I use for CSAT or CES?</summary>

Leave `threshold=-1` for the automatic top-box convention: 4+ on a 1–5 scale, 5+ on a 1–7 scale, and 9+ on a 0–10 or 1–10 scale. Override it when your survey wording defines a different success cut-off.

</details>

<details>
<summary>How is the confidence band calculated?</summary>

NPS uses the standard error for the difference between promoter and detractor proportions. CSAT uses a proportion interval for the satisfied share. CES uses the sample standard deviation around the mean. The interval is a quick normal approximation and is hidden when `confidence=none` or when there are not enough responses.

</details>

<details>
<summary>Does this replace a survey dashboard?</summary>

No. It is a local calculator for one column or tally at a time. It does not collect responses, segment by account attributes, store history, or draw trend charts. Use it to check an export, prepare a report, or verify a dashboard number.

</details>
