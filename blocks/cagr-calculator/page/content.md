## About this tool

CAGR Calculator turns a value series into a compact growth analysis. Paste two values with an elapsed-year count for the classic compound annual growth rate calculation, or paste a whole column of values to see period-over-period changes as well. Labels are optional: `2019,100000` and plain `100000` both work, and common finance formatting such as `$1,234.50`, thousands separators, and accounting negatives like `(1,234)` are accepted.

CAGR is the smoothed annual rate that takes the first value to the last value over the elapsed time. The tool also reports total growth, the growth multiple, absolute change, compound growth per period, the arithmetic mean period growth, best and worst period changes, doubling time when growth is positive, and an optional years-to-target projection. Use the spacing selector for annual, quarterly, monthly, weekly, or daily data; set **Elapsed years** for irregular spacing; or provide an exact start and end date when the measurement window matters.

### Worked example

Input values:

```text
2019,100000
2020,112000
2021,128500
2022,141000
2023,150000
```

With annual spacing, the first value is 100,000, the last value is 150,000, and the elapsed time is four years. CAGR is therefore `(150000 / 100000)^(1 / 4) - 1 = 10.67%`. The period table also shows that the year-over-year growth rates were 12.00%, 14.73%, 9.73%, and 6.38%, so the arithmetic mean period growth is not the same as the smoothed CAGR.

Use **CSV** output when you want to paste the period table into a spreadsheet, or **JSON** output when an agent or script needs the computed fields. Results are deterministic local calculations; they are educational arithmetic, not investment advice.

## FAQ

<details>
<summary>What is the difference between CAGR and average yearly growth?</summary>

CAGR is geometric: it is the single annual rate that compounds from the first value to the last value over the elapsed years. Average yearly growth is arithmetic: it averages each period's percentage change. Volatile series usually have an arithmetic average that is higher than CAGR, because losses and gains compound asymmetrically.

</details>

<details>
<summary>Do I need dates, or can I just paste values?</summary>

You can just paste values. By default, each row is treated as one annual step, so five annual values span four years. For quarterly, monthly, weekly, or daily observations, change **Spacing between values**. If the points are irregularly spaced, set **Elapsed years**, or provide both start and end dates as `YYYY-MM-DD`; those overrides take precedence over the spacing selector.

</details>

<details>
<summary>Can the input have labels, currency symbols, or a header row?</summary>

Yes. Each row may be just a number or a label plus a value, such as `2023,$150,000`. Currency symbols and thousands separators are ignored, and accounting negatives like `(1,234)` are parsed as negative numbers. Turn on **First line is a header row** for pasted CSV columns such as `year,revenue`.

</details>

<details>
<summary>Why does it reject zero or negative starting values?</summary>

CAGR uses the ratio `ending / beginning` and then takes a fractional power. That is only meaningful for a positive starting value and, for ordinary CAGR, a positive ending value. If your series crosses zero, use the period-over-period table or absolute changes instead of treating the whole range as a compound annual growth rate.

</details>

<details>
<summary>What limits should I know about?</summary>

The input needs at least two values and is capped at 2,000 points to keep browser output readable. Decimal places are limited to 0 through 10. Doubling time is shown only when CAGR is positive, and years-to-target is shown only when you provide a positive target above the last value and the computed CAGR can reach it.

</details>
