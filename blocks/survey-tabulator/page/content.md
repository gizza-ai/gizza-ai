## About this tool

Survey Tabulator turns raw survey-response data into the two tables analysts reach
for first: a **frequency table** (how many people picked each answer, and what
percent) for every question, and a **cross-tabulation** that breaks one question
down by another — with row, column, or total percentages and an optional
**chi-square test of independence** (with degrees of freedom, Cramér's V effect
size, and a p-value) to gauge whether two questions are associated.

Paste a CSV whose **first row is the question headers** and each later row is one
respondent's answers. Everything runs locally in your browser — no upload, no
account, no sending your respondents' data to a server.

### Worked example

Given this response CSV:

```
gender,satisfaction,region
F,High,North
M,Low,South
F,High,North
F,Medium,North
M,High,South
M,Low,South
F,,North
```

**Overview** of the `satisfaction` column (blank answers dropped) reports 6
responses, 1 blank, and three distinct values — `High` 3 (50.0%), `Low` 2
(33.3%), `Medium` 1 (16.7%). Switch to **crosstab** with question `gender` by
`satisfaction` and turn on stats to get a contingency table plus a line like
`Chi-square = …, df = 2, Cramér's V = …, p = …`.

### Options

- **Mode** — `overview` (per-question frequencies) or `crosstab` (two-way table).
- **Question / By** — pick columns by 1-based index or header name. In overview,
  leave *Question* blank to tabulate every column.
- **Crosstab percentages** — show each cell as a percent of the grand total, its
  row, its column, or counts only.
- **Include blank answers** — count empty cells as a `(blank)` category instead of
  dropping them.
- **Chi-square stats** — append the association test to a crosstab.
- **Sort** — order categories by count (descending) or label (alphabetical).
- **Top N** — in overview, keep only the N most frequent categories per question.
- **Delimiter** — comma, tab, semicolon, or pipe.

### Limits

- Input is parsed as text in memory; very large exports (hundreds of thousands of
  rows) may be slow but are not artificially capped.
- The chi-square p-value is the asymptotic upper-tail probability and, like any
  chi-square test, is unreliable when expected cell counts are small (a common
  rule of thumb is expected counts ≥ 5).
- Cross-tabs are two-way only (one row variable against one column variable).

## FAQ

<details>
<summary>What format does the input need to be in?</summary>

A delimited table where the **first row holds the question/column headers** and
every following row is one respondent. Comma-separated is the default; set the
*Delimiter* option to `tab`, `semicolon`, or `pipe` (or paste a single custom
character) for other exports. Fields may be quoted the usual CSV way.

</details>

<details>
<summary>How do I reference a column — by name or by number?</summary>

Either. Type the exact header text (e.g. `satisfaction`) or a **1-based index**
(`2` for the second column). Indexing from 1 matches how spreadsheets label
columns, so the first column is `1`, not `0`.

</details>

<details>
<summary>What is a cross-tabulation and when should I use one?</summary>

A crosstab (contingency table) counts how many respondents fall into each
combination of two questions — for example *gender* against *satisfaction*. It's
the standard way to see whether answers to one question differ across the groups
defined by another. Each cell can be shown as a percent of the row, the column,
the grand total, or as a raw count.

</details>

<details>
<summary>What do the chi-square, degrees of freedom, Cramér's V and p-value mean?</summary>

The **chi-square** statistic measures how far the observed cell counts are from
what you'd expect if the two questions were independent; **degrees of freedom**
is `(rows − 1) × (columns − 1)`. The **p-value** is the probability of a
chi-square that large under independence — small values (say < 0.05) suggest the
questions are associated. **Cramér's V** rescales chi-square to 0–1 so you can
judge the *strength* of that association regardless of table size.

</details>

<details>
<summary>How are blank answers handled?</summary>

By default empty cells are excluded, so percentages are out of the people who
actually answered. Turn on **Include blank answers** to keep them as an explicit
`(blank)` category and count them in the totals.

</details>

<details>
<summary>Is my survey data uploaded anywhere?</summary>

No. The tabulation runs entirely in your browser via WebAssembly — the CSV you
paste never leaves your device, so it's safe for confidential or personal
responses.

</details>
