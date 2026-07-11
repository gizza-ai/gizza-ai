## Sort a CSV by any column

Reorder the rows of a CSV or TSV by one or more columns — **ascending** or
**descending**, with **numeric-aware** or plain **lexical** ordering. The header
row (when present) stays on top and every other row is reordered around your sort
keys. Everything runs locally in your browser; nothing is uploaded.

### Worked example

Input:

```
name,age
Bob,30
Ada,36
Cy,4
```

Sort by `age` (ascending, auto ordering) →

```
name,age
Cy,4
Bob,30
Ada,36
```

Because every value in `age` is a number, **auto** ordering sorts them
numerically (`4 < 30 < 36`) rather than as text (where `"30"` would come before
`"4"`).

### Multi-column sort with per-column direction

List columns comma-separated in priority order; add a `:asc` or `:desc` suffix to
give a column its own direction. Sorting `dept:asc,salary:desc` groups rows by
department A→Z, then orders each group by salary high→low:

```
dept,salary          dept,salary
eng,100      →       eng,200
hr,50                eng,100
eng,200              hr,90
hr,90                hr,50
```

### Options

- **Sort column(s)** — header names (when the first row is a header) or 1-based
  indices, comma-separated in priority order. Each may carry a `:asc`/`:desc`
  suffix that overrides the global direction.
- **Direction** — the default `asc`/`desc` for columns without their own suffix.
- **Ordering** — `auto` sorts a column numerically when every value is a number,
  otherwise lexically; `number` forces numeric; `text` forces lexical.
- **Case-sensitive** — for text ordering, sort uppercase before lowercase
  (`Apple`, `apple`) instead of case-insensitively.
- **First row is a header** — keep it pinned on top and address columns by name.
  Turn it off to sort every row and address columns by 1-based index.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.

### Limits & edge cases

- The sort is **stable**: rows equal on every key keep their original order.
- In `auto`/`number` ordering, blank and non-numeric cells sort **after** all
  numbers in ascending order (and before them descending).
- Rows may have ragged widths; a missing cell is treated as empty.
- Everything is held in memory in your browser, so very large files are bounded
  by available RAM rather than a fixed row cap.

<details>
<summary>How do I sort by more than one column?</summary>

List the columns comma-separated in priority order, e.g. `country,city`. The
first column is the primary sort key, the next breaks ties, and so on. Add a
`:asc` or `:desc` suffix to any column to set its own direction, like
`country:asc,population:desc`.

</details>

<details>
<summary>Why did my numbers sort in a weird order?</summary>

With **text** ordering (or a column that mixes numbers and words), values sort
character by character, so `"10"` comes before `"2"`. Choose **auto** (the
default, which sorts a column numerically when every value is a number) or
**number** to force numeric ordering, and `2` will come before `10`.

</details>

<details>
<summary>Can I sort a TSV or a pipe-delimited file?</summary>

Yes. Set the **Delimiter** to `tab`, `;`, `|`, or any single character (or the
words `comma`/`tab`/`semicolon`/`pipe`). The same delimiter is used to read the
input and write the sorted output.

</details>

<details>
<summary>What happens to rows with blank or missing values?</summary>

In numeric ordering, blank and non-numeric cells sort after all numbers when
ascending (and before them when descending). In text ordering they sort as empty
strings. Rows that are shorter than the sort column are treated as having an
empty value there.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The sorting runs entirely in your browser with WebAssembly — the CSV never
leaves your device.

</details>
