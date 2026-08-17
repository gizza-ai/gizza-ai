## About this tool

Zero Pad IDs repairs the identifier column that was accidentally treated as a
number. If a spreadsheet, export job, or database loader turned `00042` into
`42`, paste the CSV/TSV table here, choose the affected column, and pad it back
to the required width without changing the rest of the row.

Worked example: with `columns = id` and `width = 5`, this input:

```csv
id,name
42,ada
7,linus
12345,grace
```

becomes:

```csv
id,name
00042,ada
00007,linus
12345,grace
```

Use `width = 0` to auto-fit each selected column to its widest eligible value,
`mode = strip` to remove leading zeros instead, and `quote_style = always` when a
downstream spreadsheet needs every padded code to look explicitly like text.
Inputs are capped at 5,000,000 bytes. Blank cells are left blank, and real digits
are never truncated to make an over-wide value fit.

## FAQ

<details>
<summary>How do I select the ID column?</summary>

With `header` enabled, use column names such as `id`, `zip`, or `account`. You
can also use 1-based positions such as `1` or `1,3`. Leaving `columns` blank
rewrites every column, which is useful for a one-ID-per-line list but risky for a
table that also contains prices or counts.

</details>

<details>
<summary>What does width 0 do?</summary>

`width = 0` means auto-fit. Each selected column is padded to the length of its
widest eligible value, so `42`, `7`, and `12345` become `00042`, `00007`, and
`12345` without you having to know the target width ahead of time.

</details>

<details>
<summary>Will it corrupt alphanumeric codes or blank cells?</summary>

By default, no. Blank cells stay blank, and values that are not plain digits,
such as `SKU-9`, `N/A`, `-42`, or `1.5`, are copied through unchanged. Set
`non_numeric = pad` only when you intentionally want to pad alphanumeric codes.

</details>

<details>
<summary>How do I stop spreadsheets from removing the zeros again?</summary>

Set `quote_style = always` so every output field is quoted, then import the
column as text in your spreadsheet or loader. Quoting helps many readers preserve
the string form, but a program that forcibly casts the field to a number can
still remove leading zeros.

</details>
