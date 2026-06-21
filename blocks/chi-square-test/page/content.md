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
