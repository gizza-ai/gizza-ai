## What this tool does

**Column Value Diff** joins two tables on a **key column** and compares a single
**value column** between the matched rows — reporting every key whose value changed as a
clean `old → new` pair. Every other column is ignored, so two exports with different
surrounding columns still reconcile cleanly on the one metric you care about: a price, a
quantity, a status, a balance.

It's the focused answer to "did the value for this id change between yesterday's file and
today's?" — where a full cell-by-cell diff would drown you in unrelated column noise.

## Worked example

**Original CSV**

```
id,name,price
1,Apple,10
2,Banana,20
3,Cherry,30
```

**Updated CSV**

```
id,name,price
1,Apple,12
2,Banana,20
3,Cherry,35
```

With **key = `id`** and **value = `price`**, the table report is:

```
value column "price" · 3 keys matched · 2 changed · 1 unchanged
~ [1] "10" → "12"
~ [3] "30" → "35"
```

Row `2` (Banana) is unchanged, and the `name` column is never compared. Switch the output
to **CSV change-log** for a flat, pasteable result:

```
key,status,old,new
1,changed,10,12
3,changed,30,35
```

Turn on **Include keys on only one side** to also list keys that appear in just one file
(`left_only` / `right_only`) — useful for spotting added or dropped rows alongside the value
changes.

## How to use it

1. Paste the **original** (old) CSV and the **updated** (new) CSV.
2. Enter the **key column(s)** that identify a row — e.g. `id`, or `first,last` for a
   composite key. Reordered rows still match, because rows are joined by key, not by line.
3. Enter the **value column** to compare — the one metric, e.g. `price`.
4. Pick the delimiter, header, and matching options, then read the `old → new` report.

Everything runs locally in your browser — the CSVs are never uploaded.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>How is this different from a full CSV diff?</summary>

A full cell diff compares **every** column and flags every changed cell, plus whole columns
that were added or removed. That's noisy when the two files have different surrounding
columns. This tool compares **one** value column, matched by key, so you get just the
`key → old/new` changes for the metric you care about — nothing else.

</details>

<details>
<summary>Can I match on more than one column?</summary>

Yes. Enter a **composite key** as a comma-separated list, e.g. `first,last` or
`store,sku`. Rows match only when every key column agrees. Duplicate keys are paired in
their original order.

</details>

<details>
<summary>What if a key exists in only one of the files?</summary>

By default those rows are ignored, so the report shows only value changes for keys present
in **both** tables. Turn on **Include keys on only one side** to also list them as
`left-only` (only in the original) and `right-only` (only in the updated) with their value.

</details>

<details>
<summary>My CSV has no header row — can I still use it?</summary>

Yes. Turn off **First row is a header** and reference the key and value columns by their
**1-based index** instead of a name — e.g. key `1`, value `3`. Columns are labelled
`col1`, `col2`, … internally.

</details>

<details>
<summary>Does it treat "10" and "10.0" (or different case) as equal?</summary>

Values are compared as **text**, so `10` and `10.0` are different, and `Yes` differs from
`yes` unless you enable **Ignore case**. Enable **Ignore whitespace** to fold runs of spaces
(so `in  stock` matches `in stock`). Both options affect matching only — the report always
shows the original text.

</details>

## Limits & edge cases

- Comparison is **textual** — there's no numeric tolerance or rounding; `10` ≠ `10.0`.
- The **key** and **value** columns are required and must exist in **both** tables (by name,
  or by 1-based index when header is off), otherwise you get a clear "not found" error.
- Duplicate keys pair in order (1st-with-1st, 2nd-with-2nd); leftover duplicates on one side
  surface as unmatched when **Include keys on only one side** is on.
- Supported delimiters: comma, tab, semicolon, pipe, or any single character. Quoted fields
  and embedded commas/newlines are parsed correctly.
- Processing is entirely in-browser (wasm); large files are handled in memory with no upload.
