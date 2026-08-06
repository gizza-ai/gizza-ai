## Clean spreadsheet numbers without guessing by hand

Exports from accounting systems, dashboards, PDFs and vendor spreadsheets often look numeric to a human but are text to a machine: `$1,234.50 USD`, `1 234,56`, `(250.00)`, `45.2%`, `1.2K`, or values with non-breaking spaces pasted from the web. Paste a column here and get back plain floats that sort, sum and import cleanly.

The sanitizer works one row per line. It strips currency symbols, thousands separators, common units, Unicode whitespace, accounting parentheses, trailing minus signs and optional K/M/B/T finance suffixes. In `auto` mode it infers one decimal convention for the whole column so European and US formats are handled consistently.

**Worked example:** paste this messy column:

```text
$1,234.50 USD
(250.00)
1.2K
45.2%
```

With defaults you get:

```text
1234.5
-250
1200
45.2
```

Choose **Percent signs → Divide by 100** when you need `45.2%` as `0.452` for calculations, or **Output format → Audit table** when you want to keep each original value next to its parsed result and status.

### Options

- **Decimal separator** — `auto` infers dot vs comma for the whole column; force `dot` for `1,234.56` or `comma` for `1.234,56`.
- **Percent signs** — strip the percent sign or divide the value by 100.
- **Expand K/M/B/T suffixes** — `1.2K`, `3M`, `2bn`, and `1T` become full numbers; ordinary units like `kg` are stripped without scaling.
- **Accounting parentheses are negative** — `(250.00)` becomes `-250`.
- **Round decimals** — keep full precision or round to a fixed number of places.
- **Rows that cannot be parsed** — emit blank lines, keep the original text, write `#ERROR`, or fail immediately.
- **Output format** — values only, TSV audit table, or JSON with row statuses and totals.
- **Append summary stats** — add count, parsed/failed/empty, sum, min, max and average.

### Notes and limits

- Maximum input is 20,000 rows per run.
- Output is always a dot-decimal numeric string; grouping separators are never emitted.
- Empty middle rows stay aligned with the source. Trailing blank lines are ignored.
- Values too large for a 64-bit float are rejected.
- Auto decimal detection assumes the column uses one convention. If your data mixes `1,234.56` and `1.234,56`, force the intended convention or split the column first.

## FAQ

<details>
<summary>Will this change my decimal comma data into dot decimals?</summary>

Yes. The output is meant for software pipelines and always uses dot decimals, so `1.234,56` becomes `1234.56`. Use the decimal separator option if auto-detection does not match your source convention.

</details>

<details>
<summary>What happens to rows like `n/a` or `—`?</summary>

That depends on the error policy. The default leaves a blank output row so spreadsheet alignment is preserved. You can instead keep the original text, emit `#ERROR`, or fail the whole run on the first bad row.

</details>

<details>
<summary>Does `45%` become `45` or `0.45`?</summary>

By default it becomes `45`, because many cleanup tasks only need the percent sign removed. Switch **Percent signs** to **Divide by 100** when you need fractional values for formulas or model features.

</details>

<details>
<summary>Are units like `kg`, `ms`, or `USD` multiplied?</summary>

No. Ordinary units and currency codes are stripped. Only common finance magnitude suffixes are expanded when that option is on: `K`, `M`, `B`/`bn`, and `T`/`tn`.

</details>

<details>
<summary>Can I audit which rows failed?</summary>

Yes. Choose the TSV audit table or JSON output. Both include each original row, the parsed value when available, and an error status for rows that could not be parsed.

</details>
