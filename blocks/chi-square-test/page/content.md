## About this tool

The **chi-square test** checks whether observed category counts differ from what
you'd expect under a null hypothesis. This tool runs both common variants of
**Pearson's χ²** and reports the statistic, **degrees of freedom**, and the
**p-value**.

### Goodness-of-fit

Compare a single row of **observed** counts against an **expected** distribution.
Leave the expected box blank for an equal-frequency (uniform) null — e.g. testing
whether a die is fair. Or supply expected counts or **ratios** (like `9 3 3 1` for
a Mendelian cross); ratios are automatically rescaled to the observed total.
Degrees of freedom = *k* − 1, where *k* is the number of categories.

### Contingency table (test of independence)

Pick **contingency** and paste an **r × c table** of observed counts — one row per
line, cells separated by spaces, commas, or tabs. The test asks whether the row
and column variables are independent. Expected counts are
row-total × column-total ÷ grand-total, degrees of freedom = (*r* − 1)(*c* − 1),
and the tool also reports **Cramér's V** as an effect-size measure (0 = no
association, 1 = perfect association). For a **2×2** table you can enable
**Yates' continuity correction**, which shrinks each |O−E| by 0.5 before
squaring for a more conservative statistic.

### Reading the result

A small p-value (conventionally **p < 0.05**) means the observed counts are
unlikely under the null, so you **reject** it. The tool flags any cell whose
**expected count is below 5**, where the chi-square approximation becomes
unreliable.

### Privacy

Everything runs **in your browser** via WebAssembly — your data is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the values as
structured JSON).

## FAQ

<details>
<summary>Can I enter expected ratios instead of exact counts?</summary>

Yes. In goodness-of-fit mode the expected box accepts either counts or ratios —
whatever you type is **rescaled so it sums to the observed total**, so `9 3 3 1`
works directly for a Mendelian 9:3:3:1 cross. The only requirements are that
every expected value is positive and that the number of expected values matches
the number of observed categories.

</details>

<details>
<summary>Why is Yates' continuity correction being ignored for my table?</summary>

Yates' correction only makes sense for a **2×2** contingency table, so the tool
applies it there and silently skips it for anything larger (a 3×4 table gets
the plain Pearson statistic even with the box ticked). It also has no effect in
goodness-of-fit mode.

</details>

<details>
<summary>What does the "expected count below 5" warning mean?</summary>

The chi-square p-value is an approximation that breaks down when expected cell
counts are small. The tool counts how many cells have an expected value under
5 and flags them; if any are flagged, treat the p-value as rough — for small
2×2 tables consider Fisher's exact test instead.

</details>

<details>
<summary>How do I format a contingency table?</summary>

One row per line, with cells separated by spaces, commas, or tabs. Every row
must have the same number of columns, and the table needs at least 2 rows and
2 columns; counts must be non-negative and can't all be zero.

</details>
