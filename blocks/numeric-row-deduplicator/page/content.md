## About this tool

The **Numeric Row Deduplicator** removes duplicate rows from a numeric table (CSV or TSV), but
unlike an ordinary text deduplicator it compares each cell by its **numeric value**. That means
`1`, `1.0`, `1.00`, `+1`, `1e0` and `100e-2` are all recognised as the same number, so rows that
only *look* different collapse into one. A plain string-based deduper would keep every one of them.

Paste your rows, optionally pick the columns to key on, and the tool keeps the first occurrence of
each numeric row (in its original order) and drops the rest. Everything runs locally in your
browser — nothing is uploaded.

### Worked example

Input (one row per line):

```
1,2
1.0,2.0
1.00,2
3,4
```

Output (`De-duplicated rows`):

```
1,2
3,4
```

Rows 1–3 are the same pair of numbers written three ways, so only the first survives; the `3,4`
row is unique and is kept.

### What you can control

- **Key columns** — match on a subset of columns by 1-based number (`1,3`) or, with a header, by
  name (`id,region`). Blank keys on the whole row.
- **Header** — when on, the first row is kept verbatim, excluded from the scan, and its names can be
  used as key columns.
- **Delimiter** — `,` (default), `tab`, `;` (semicolon) or `|` (pipe), or any single character.
- **Rounding tolerance** — round every numeric cell to N decimals before comparing so float-noise
  near-duplicates collapse (`0.30000000000000004` and `0.3` are equal at 2 decimals). `-1` compares
  the exact value.
- **Keep first / last** — keep the earliest or the latest occurrence of each duplicate; either way
  the surviving rows stay in their original order.

### Limits & behaviour

- Comparison is by value: two cells are equal when they parse to the same `f64` (after any
  rounding). Non-numeric cells (text, currency symbols, blanks) fall back to a trimmed-string
  compare, so mixed tables still de-duplicate sensibly.
- Rounding tolerance accepts 0–12 decimal places (or `-1` for exact).
- Very large numbers beyond double-precision range and special tokens like `NaN`/`Inf` are treated
  as text, not numbers.
- This is exact numeric equality, not fuzzy matching — for approximate string matching use a fuzzy
  deduplicator instead.

## FAQ

<details>
<summary>How is this different from a normal CSV duplicate remover?</summary>

A normal remover compares rows as raw text, so `1.0` and `1.00` are two different rows and both
survive. This tool parses each cell as a number and compares the **values**, so every textual form
of the same number collapses to a single row. That is the whole point of a *numeric* deduplicator.

</details>

<details>
<summary>Can I de-duplicate on just some columns?</summary>

Yes. Put the columns you want to key on in **Key columns** — 1-based numbers like `1,3`, or header
names like `id,region` when **Header** is on. Rows that match on those columns are treated as
duplicates even if other columns differ, and the first (or last) matching row is kept.

</details>

<details>
<summary>What does the rounding tolerance do?</summary>

Set it to a number of decimal places and every numeric cell is rounded to that precision *before*
comparing. This collapses near-duplicates caused by floating-point noise — for example
`0.30000000000000004` and `0.3` become equal at 2 decimals. Leave it at `-1` to compare the exact
value with no rounding.

</details>

<details>
<summary>What happens to non-numeric cells?</summary>

Cells that do not parse as a number (labels, currency symbols, blanks, `NaN`) are compared as
trimmed text instead. So a table mixing a `region` column with numeric measurements still
de-duplicates correctly — the text columns match by text, the number columns match by value.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely inside your browser tab. Your rows never
leave your device, which makes it safe for sensitive or proprietary data.

</details>
