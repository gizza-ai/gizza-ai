## CSV coalesce columns

Build **one** column from the first non-empty value across several columns, read
in the priority order you list them — the SQL `COALESCE` idea applied to columns
instead of expressions. It's the fastest way to fold `mobile`/`office`/`home`
into a single `phone`, or a web price with a list-price fallback, without writing
a spreadsheet formula. Optionally drop the source columns, choose where the new
column lands, and set a fallback for rows where every source is empty. Runs
entirely in your browser; nothing is uploaded.

### Worked example

Input — three phone columns, filled in unevenly:

```
name,mobile,office,home
Ann,555-1,555-2,555-3
Bob,,555-4,555-5
Cleo,,,555-6
```

Source columns `mobile,office,home`, new column name `phone` → output:

```
name,mobile,office,home,phone
Ann,555-1,555-2,555-3,555-1
Bob,,555-4,555-5,555-4
Cleo,,,555-6,555-6
```

Ann keeps her mobile (first in the priority list), Bob falls through to the
office number, Cleo all the way to home. Turn on **Drop the source columns** and
set the position to *where the first source column was* to get the tidy version
instead:

```
name,phone
Ann,555-1
Bob,555-4
Cleo,555-6
```

### Options

- **Source columns** — comma-separated, **highest priority first**: header names
  (`mobile,office,home`) or 1-based indices (`2,3,4`). A purely numeric token is
  always read as an index, so a column literally named `2` must be addressed by
  its position.
- **New column name** — blank uses `coalesced`. It must not clash with a column
  you keep.
- **Put the new column** — at the end, at the start, or where the first source
  column sat.
- **Fallback** — written when *every* source is empty for that row (`N/A`,
  `unknown`, …). Blank leaves the cell empty.
- **Drop the source columns** — removes them after merging, so only the new
  column remains.
- **Treat whitespace-only cells as empty** — on by default, so a stray space is
  skipped rather than winning.
- **Extra placeholders that count as empty** — comma-separated tokens such as
  `NULL,NA,N/A,-`, matched case-insensitively against the trimmed cell.
- **First row is a header** — keeps and rewrites the header row, and lets you
  name columns instead of counting them.
- **Delimiter** — comma, tab, semicolon, pipe, or any single character.

### Limits & edge cases

- Only *emptiness* decides the winner — no type checks, no `0`/`false` special
  case. A cell containing `0` is a real value and wins.
- Placeholders like `NULL` or `N/A` are ordinary text unless you list them under
  **Extra placeholders that count as empty**.
- Rows shorter than the widest row are padded with empty cells, so a missing
  trailing column simply falls through to the next source.
- Listing the same column twice, naming a column that isn't in the header, or
  using an index past the last column is an error rather than a silent skip.
- The new column's name must not collide with a column you keep — rename it, or
  drop the sources.
- Values are copied verbatim (quotes, inner commas, unicode); the tool never
  reformats or trims the value it picks.

### FAQ

<details>
<summary>How is this different from concatenating or merging columns?</summary>

Concatenating joins *every* value together (`555-1 / 555-2`). Coalescing picks
exactly **one** — the first source that has a value for that row — and ignores
the rest. Use it when the columns are alternatives for the same fact, not parts
of one.

</details>

<details>
<summary>What counts as an empty cell?</summary>

A zero-length cell always counts. With **Treat whitespace-only cells as empty**
on (the default), a cell holding only spaces or tabs counts too. Anything you
list under **Extra placeholders that count as empty** — for example
`NULL,NA,N/A,-` — also counts, compared case-insensitively after trimming, so
`n/a` and `N/A` both match.

</details>

<details>
<summary>Can I keep the original columns?</summary>

Yes — that's the default. The new column is added alongside them, so you can
check the result before deleting anything. Turn on **Drop the source columns**
only when you want them replaced; other columns are always kept in their
original order.

</details>

<details>
<summary>Does it work without a header row?</summary>

Yes. Switch **First row is a header** off and address columns by 1-based index
(`2,3,4`). Every row is then treated as data, and the coalesced column is added
without a header cell.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — the CSV is processed locally with WebAssembly. Nothing leaves your browser.

</details>
