## About this tool

One-Hot Encoder expands a categorical CSV column into a block of binary
indicator columns — one per distinct value — where every row carries a **1** in
the column matching its category and a **0** in all the others. This is the
transformation usually called **one-hot encoding** or **dummy variables**, and
it is what most modelling libraries produce from a text column before it can be
fed to a regression, a tree ensemble, or a neural net.

Pick the column and the tool does the rest: generated columns are named
`<prefix><separator><value>` (the prefix defaults to the column's own name), and
you can drop a **reference level** to avoid the dummy-variable trap, cap the
expansion to the **top N** most frequent categories, ignore values seen fewer
than N times, keep or remove the source column, and write `true`/`false` (or any
other pair) instead of `1`/`0`.

### Worked example

CSV:

```csv
city,n
Paris,1
Rome,2
Paris,3
```

With column `city` and the defaults, the `city` column is replaced by one
indicator per distinct value, in alphabetical order:

```text
n,city_Paris,city_Rome
1,1,0
2,0,1
3,1,0
```

Set **Reference level to drop** to *First category* and the `city_Paris` column
disappears — the rows that were Paris are now identified by being 0 in every
remaining column, which is the k−1 encoding a linear model needs:

```text
n,city_Rome
1,0
2,1
3,0
```

### Capping a high-cardinality column

Columns like browser, merchant, or ZIP can hold thousands of distinct values, and
one column each is rarely useful. **Keep only the top N categories** selects the
N most frequent, and **Add a combined 'other' column** collects everything else
into a single indicator. With `max_categories = 2`, `other_column` on, the
original column kept, and frequency ordering:

```csv
browser,hits
chrome,10
chrome,20
chrome,30
safari,40
safari,50
lynx,60
```

```text
browser,hits,browser_chrome,browser_safari,browser_other
chrome,10,1,0,0
chrome,20,1,0,0
chrome,30,1,0,0
safari,40,0,1,0
safari,50,0,1,0
lynx,60,0,0,1
```

## Limits & edge cases

- Categories come from this one input only. There is no separate fit/transform
  split, so "unseen category" handling does not apply — every value present in
  the data gets encoded as it is read.
- **At most 512 indicator columns** are generated. A higher-cardinality column
  fails with an explicit error; use *top N categories* or the minimum count to
  bring it under the limit rather than encoding an ID column.
- One column per run. Columns are chosen by header name, or by 1-based number
  when the header checkbox is off. With no header there is no header row in the
  output either, so the generated column names are not written.
- Indicator columns are always **appended at the end** of each row, in the chosen
  order, followed by the `other` column and then the `NaN` column when those are
  enabled. Dropping a reference level applies to the category columns only —
  the `other` and `NaN` buckets are never the dropped level.
- **Top N always selects by frequency**, even when the column order is set to
  alphabetical or first-seen; the ordering setting only decides how the surviving
  columns are arranged. Ties are broken by which value appeared first.
- Values are trimmed before grouping; with case-sensitivity off they are also
  lower-cased, and the resulting column is named after the **first spelling
  seen**. The original cell text is untouched when the source column is kept.
- Category values containing the delimiter, quotes, or newlines are quoted in the
  generated header exactly as CSV requires, so `Paris, FR` becomes the quoted
  column `"city_Paris, FR"`.

## FAQ

<details>
<summary>What is one-hot encoding, and when should I use it?</summary>

One-hot encoding turns a categorical column into several binary columns, one per
category, so that a model can use it without inventing an ordering. If you simply
numbered categories 1, 2, 3 instead, most models would read that as "3 is bigger
than 1", which is meaningless for values like city or browser. Use it for
**nominal** columns with a manageable number of distinct values; for
high-cardinality columns, cap it with the top-N setting or reach for a compact
alternative such as frequency or target encoding.

</details>

<details>
<summary>What is the dummy-variable trap, and which drop option fixes it?</summary>

With one column per category, the indicators always sum to 1 for every row, so
any one of them is perfectly predictable from the others. That perfect
collinearity makes a linear regression's coefficients unstable or unsolvable.
Dropping one category as a **reference level** fixes it: pick *First category* or
*Last category* to get k−1 columns, and rows in the dropped category are then the
ones that are 0 everywhere. *Only if the column is binary* drops a level only when
there are exactly two categories, which is the common convention for yes/no
columns. Tree-based models do not care, so *None* is a fine default for them.

</details>

<details>
<summary>How are blank cells handled?</summary>

The **Blank cells** setting decides. *Zero in every indicator* (the default)
treats a blank as "none of the above", which matches what most encoder libraries
do by default. *Own NaN indicator column* adds a `<prefix>_NaN` column, so
missingness itself becomes a feature you can model. *Leave the indicators empty*
writes empty cells so the gap stays visible downstream, and *Reject the input*
fails with an error rather than silently choosing for you. Blank rows never get a
category column of their own unless you choose the separate option.

</details>

<details>
<summary>How do I get true/false or Y/N instead of 1 and 0?</summary>

Set **Value for a match** and **Value for a non-match** to whatever pair you
need — `true`/`false`, `Y`/`N`, or `yes`/`no`. They are written verbatim, so the
output plugs straight into a tool that expects booleans rather than integers.

</details>

<details>
<summary>What happens to categories that the top-N or minimum-count limits exclude?</summary>

By default those rows are simply 0 in every generated column, exactly as if the
value had never been listed. Turn on **Add a combined 'other' column** to give
them a single shared indicator instead, so the information that the row held
*some* rare value is preserved in one column rather than lost or spread across
many. The two limits combine: a category must both be seen at least the minimum
number of times and survive the top-N cut to get its own column.

</details>

<details>
<summary>Can I encode more than one column at a time?</summary>

Not in a single run — the tool takes one column per call. To encode several,
run it repeatedly, feeding each run's output back in as the next run's input and
changing only the column name. Because the generated columns are appended at the
end and the source column is removed by default, the results stack cleanly
without colliding.

</details>
