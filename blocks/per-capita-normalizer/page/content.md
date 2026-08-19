## About this tool

Raw counts lie when the groups behind them are different sizes. A city of two million will report more cases, crimes, complaints or sign-ups than a town of forty thousand even when the underlying risk is identical. Per Capita Normalizer divides each count by its own population and rescales it to a shared reporting base, so the numbers become comparable.

Paste one `label, count, population` row per line, pick a base — per person, per 1,000, per 10,000, per 100,000 (the public-health convention), per 1,000,000, or a custom base such as "per 500 residents" — and the tool returns a ranked table with:

- **rate** — the count divided by population, scaled to your base;
- **index** — that rate against the combined rate of every row you pasted, where `1.00` is the overall average, `2.00` is twice the average and `0.50` is half;
- **flag** — `unstable` when a row's raw count is below your small-count threshold (20 by default), the usual warning that a rate built on a handful of events swings wildly.

Everything runs locally in your browser; nothing is uploaded.

### Worked example

Input (comma-delimited, first row is a header):

```text
region,cases,population
Northbridge,120,400000
Eastvale,45,150000
Westport,18,900000
```

Output at `per 100,000` with 2 decimals:

```text
per 100000 · rows: 3 · total count: 183 · total population: 1450000 · overall rate: 12.62 · flagged unstable (count < 20): 1

rank	label	count	population	rate_per_100000	index	flag
1	Eastvale	45	150000	30.00	2.38	ok
2	Northbridge	120	400000	30.00	2.38	ok
3	Westport	18	900000	2.00	0.16	unstable
```

Northbridge has by far the biggest raw count, but Eastvale — under half its size — carries the exact same rate of 30 per 100,000. Westport looks busy at 18 cases until you notice it is spread across 900,000 people, giving a rate 84% below the overall average, and its 18 events sit under the 20-event threshold, so it is flagged as unstable.

### Reading population tables that aren't in people

International statistics often publish population in thousands or millions. Set **Population column is in** to `Thousands` or `Millions` and the tool multiplies the column for you, instead of forcing you to expand `8,175` into `8175000` by hand.

### Limits and conventions

- Up to 10,000 rows per run; each row needs a count of zero or more and a population greater than zero.
- The last two fields on a row are read as count and population, so labels may contain the delimiter (`Springfield, IL,10,1000` works).
- Numbers may include `$`, `£`, `€`, thousands separators and underscores — but with the comma delimiter, a value written `8,175,133` splits into separate fields, so use tabs (or paste straight from a spreadsheet) for comma-separated numbers.
- Counts and population must describe the same period. Mixing a full year of events with a mid-year population is fine; mixing three years of events with one year of population inflates the rate.
- These are **crude** rates: they do not adjust for age or any other structure in the population. A retirement town and a university town can differ on crude rates while having identical risk at every age.

## FAQ

<details>
<summary>Why per 100,000 rather than a percentage?</summary>

A percentage is just a rate per 100. For events that are rare relative to the population — disease cases, homicides, fatal accidents — a percentage collapses into a string of zeros (`0.003%`), while per 100,000 gives a readable `3.0`. Pick the base that puts your typical value in the 1–1000 range: per 1,000 for births, deaths or defects, per 100,000 for rarer events, per person for things everybody does several times.

</details>

<details>
<summary>What does the index column mean?</summary>

It is each row's rate divided by the overall rate of everything you pasted, so `1.00` means "exactly the pooled average". A row at `2.38` is 138% above average; a row at `0.16` is 84% below. It saves you a second pass of mental arithmetic when comparing a region against the group instead of against the top row.

</details>

<details>
<summary>Why is a row flagged unstable?</summary>

Rates built on very few events are noisy: with 3 events, one more or fewer moves the rate by a third. Statistical agencies commonly suppress or asterisk rates under about 20 events for that reason, so this tool flags them by default. Change the threshold in **Flag rows with a count below**, or set it to `0` to turn flagging off entirely — every row then reads `ok`.

</details>

<details>
<summary>Can I paste straight from a spreadsheet?</summary>

Yes. Copy the three columns and leave the delimiter on Auto (or choose Tab). Auto-detect also handles semicolon and pipe files, and falls back to whitespace when a row contains no separator at all. Header rows are detected automatically when the count or population cell isn't a number, and you can force the choice with the Header row control.

</details>

<details>
<summary>What if my rows have no label?</summary>

A two-field row is read as `count, population` and labelled `row 1`, `row 2`, and so on, so you can paste two bare columns. Single-row input works too — the index is then `1.00`, since the one row is the whole population.

</details>

<details>
<summary>Does it do age-standardized rates?</summary>

No. It computes crude rates only. Age standardization needs an external standard population broken into age bands and the same age breakdown for your own data, which is a different input shape. If you need age-adjusted comparisons, compute the rate per age band here and weight them yourself.

</details>

<details>
<summary>How do I get the result into a report?</summary>

Choose the output format: `CSV` for a spreadsheet (it starts with a small metric block, then the row table), `Markdown` for a documentation or ticket table, or `JSON` for scripts and dashboards. The text format adds a fixed-width rate chart for a quick visual scan, and every format can be copied with one click.

</details>
