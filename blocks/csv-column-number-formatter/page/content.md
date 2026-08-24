## About this tool

A number column that came out of a database, an export or somebody's spreadsheet is almost never uniformly formatted. One row has `1234.5`, the next has `7`, a third has `1,234.50`, and one poor cell says `n/a`. Sorting still works, but everything downstream — a printed report, a PDF, a chart axis, a diff against last month's file — reads badly, and a naive "round it in a formula" pass introduces a second problem: `1.005` at two decimal places comes back as `1.00` in almost every spreadsheet and scripting language, because `1.005` has no exact binary representation.

This tool gives the columns you pick **one uniform format**. You choose how many decimal places every value carries, how a dropped digit is resolved, whether digits are grouped, which characters separate them, how the sign is shown, and what wraps around the number. The table is parsed first, so row order, column count and the delimiter survive intact, quoting is re-derived from the new values, and every column you did not select is copied through byte-for-byte.

Rounding runs on the **digit string** that was parsed out of the cell — nothing is ever converted to a binary float — so the half-way cases land where you expect them to.

### Worked example

Input, with `price` as the only column in scope, **Decimal places** `2`, **Digit grouping** thousands and **Prefix** `$`:

```text
sku,price,weight
A1,1234.5,0.5
B2,7,12.25
```

Output:

```text
sku,price,weight
A1,"$1,234.50",0.5
B2,$7.00,12.25
```

Three things to notice. `weight` was not selected, so `0.5` stays `0.5` rather than becoming `0.50`. The grouped value contains a comma, so it is quoted automatically — the CSV still parses as three columns. And `$7.00` is not quoted, because it does not need to be; quoting is derived from each new value, not carried over from the input.

Switch **Output** to the audit report and you get counts instead of data:

```text
column,cells_formatted,cells_unchanged,non_numeric
price,2,0,0
TOTAL,2,0,0
```

That is the safe way to try a format on a file you care about — check that the number of cells it would touch is the number you expected, and that `non_numeric` is zero if you believed the column was clean. **Only the rows that changed** is the middle ground: the header plus the affected rows, so you can read the actual before-and-after values for a handful of them.

### Decimal places, including negative ones

**Decimal places** is how many digits every value carries after the decimal mark. Values are padded as well as rounded, which is the point: `7` becomes `7.00` so the column lines up. `0` gives whole numbers.

The field also accepts **negative** values, which round to the *left* of the decimal point:

| Decimal places | `12345` becomes |
| --- | --- |
| `2` | `12345.00` |
| `0` | `12345` |
| `-1` | `12350` |
| `-2` | `12300` |
| `-3` | `12000` |

The range is `-9` to `15`.

### Rounding modes

- **Half up** (default) — the spreadsheet `ROUND`. `2.5` → `3`, `-2.5` → `-3`.
- **Half down** — an exact half goes toward zero. `2.5` → `2`, but `2.51` → `3`.
- **Half to even** — banker's rounding. `0.5` → `0`, `1.5` → `2`, `2.5` → `2`. Use it when a long column of rounded values must not drift upward in aggregate.
- **Ceiling** / **Floor** — always toward `+∞` / `-∞`, so `-2.9` ceilings to `-2` and floors to `-3`.
- **Truncate** — drop the extra digits without looking at them. `2.9` → `2`, `-2.9` → `-2`.

### Reading messy input

Cells do not have to be clean before you start. A currency symbol on either end, group marks (`,` `.` space `_` `'`), accounting parentheses, a trailing minus in ledger style, a Unicode minus, a trailing `%` and scientific notation are all understood — `$1,234.50`, `(250)`, `250-`, `−12`, `1.5e3` and `.5` all parse.

The one genuine ambiguity is `1,234`: one thousand two hundred and thirty-four, or one and a bit? **Decimal mark in the INPUT cells** settles it. On `auto` the rule is: when both `.` and `,` appear, whichever comes last is the decimal mark; a lone `.` is a decimal point; a lone `,` followed by exactly three digits is grouping, and otherwise it is a decimal comma (`0,5` is a half). Set it explicitly to **Dot** or **Comma** when a whole column has to be read the same way regardless of what each cell looks like.

Because the input side and the output side are separate settings, a European column can be read as `1.234,56` and written back as `1234.56`, or the other way round.

### Controls

- **Columns** — blank or `*` for every column; otherwise header names, 1-based indices and inclusive `2-4` ranges, comma-separated and mixable (`price,3,5-7`). Names match exactly first, then case-insensitively.
- **Decimal places** and **Rounding** — as above.
- **Notation** — standard digits (default), compact `1.23M`, scientific `1.23e+6`, or percent (multiplies by 100 and appends `%`). Decimal places applies to the rendered mantissa in each case.
- **Digit grouping** — none (default), thousands (`1,234,567`), or Indian (`12,34,567`).
- **Group separator** — comma, period, space, thin space (U+202F), apostrophe, or underscore. Only used when grouping is on.
- **Decimal separator (output)** — period (default) or comma.
- **Sign style** — minus on negatives only (default), always signed, `+` on positives but not zero, no sign at all, a leading space where the `+` would go so a column lines up, or accounting parentheses.
- **Prefix / Suffix** — text wrapped around the number. The prefix sits *inside* the sign, so `-$1,234.00` and `($1,234.00)` both come out right. Both are inserted verbatim: include your own space in a suffix like `" kg"`.
- **Cells that are not numbers** — keep them (default), blank them, or stop with an error naming the row, column and value.
- **First row is a header** — on by default: the header's names can be used in **Columns**, and the header itself is never reformatted, so a column called `2024` stays `2024`.
- **Delimiter** — `auto` (default) sniffs it from the first line, counting candidates outside quotes with comma winning a tie; or `comma`, `tab`, `semicolon`, `pipe`, or any single character. The output uses the same separator.
- **Output quoting** — minimal (default), always, or everything except numbers.
- **Output** — the formatted CSV, only the changed rows, or the per-column audit report.

### Limits and edge cases

The table is capped at 5,000,000 bytes and a single value at 4,096 digits. An **empty cell stays empty** under every non-numeric policy — a missing value is not a zero, and inventing `0.00` would be a data change. A value that rounds to zero never keeps a minus sign, so you never see `-0.00`. Ragged rows are preserved rather than padded. Turning on grouping makes the column no longer parse as a number in the next tool you feed it to, which is exactly why grouping is **off** by default. Everything runs locally in your browser; the table is never uploaded.

## FAQ

<details>
<summary>Why does rounding 1.005 to two places give 1.01 here and 1.00 almost everywhere else?</summary>

Because almost everywhere else the value has already been through a binary floating-point number by the time rounding happens. `1.005` cannot be represented exactly in binary; the nearest `double` is very slightly *below* it, so the usual `round(x * 100) / 100` sees something like `1.00499999999999989` and correctly rounds it down. The same effect turns up as `2.675 → 2.67` and `8.475 → 8.47`.

This tool never builds a float. The digits are parsed out of the cell into an exact decimal — a sign, a digit string and a scale — and rounding is a decision about which digits to drop, made by looking at the first dropped digit. `1.005` at two places therefore rounds on the literal `5` and gives `1.01`, which is what a person checking your invoice column by hand would also write.

</details>

<details>
<summary>What is banker's rounding and when should I use it?</summary>

Half-to-even rounds an exact half toward whichever neighbour is even: `0.5 → 0`, `1.5 → 2`, `2.5 → 2`, `3.5 → 4`. Anything that is not an exact half rounds normally, so `2.51` still goes to `3`.

It matters when you round a long column and then total it. Half-up sends every single exact half upward, so a column full of `.5` values gains a systematic bias — round a thousand of them and the total is meaningfully too high. Half-to-even splits those cases roughly evenly between up and down, which keeps the rounded total close to the unrounded one. Accounting and statistical work usually specifies it; a price list a human reads usually does not care, which is why half-up is the default here.

</details>

<details>
<summary>Can I read a European column and write a US one, or the reverse?</summary>

Yes — the input convention and the output convention are separate controls, deliberately. **Decimal mark in the INPUT cells** says how to *read* `1.234,56`; **Group separator** and **Decimal separator (output)** say how to *write* the result.

So to convert a German export to US convention, set the input decimal mark to Comma, grouping to Thousands, the group separator to Comma and the output decimal separator to Period. To go the other way, set the input decimal mark to Dot, the group separator to Period and the output decimal separator to Comma, and `1234.56` comes back as `1.234,56`. Setting the input mark explicitly rather than leaving it on auto is worth doing whenever a column contains values like `1,234` whose meaning depends entirely on the convention.

</details>

<details>
<summary>Why is digit grouping off by default when every other number formatter turns it on?</summary>

Because the other formatters print a number for a person to read, and this one writes a CSV that something else usually reads next. `1,234,567` in a comma-delimited file has to be quoted to survive the round trip — the tool does quote it automatically — but it also stops being a *number* as far as the next importer, database load or charting step is concerned. Turning grouping on is a decision to make the file presentational, so it is opt-in rather than the default.

When you do want it, both common patterns are there: thousands groups by three from the right, and Indian groups the last three digits and then in twos (`12,34,567`). If the grouped output is destined for a comma-delimited file, consider a group separator of space or thin space, or switch the delimiter to tab or semicolon so the quoting is not needed at all.

</details>

<details>
<summary>What happens to cells that are not numbers, or that are empty?</summary>

An **empty** cell is always left empty, in every policy. A blank is a missing value; writing `0.00` into it would silently invent data, and no option here does that.

A cell with content that is not a number — `n/a`, `pending`, `TBD` — is governed by **Cells that are not numbers**. *Keep* (the default) copies it through untouched, which is right when the column legitimately mixes values and markers. *Blank* replaces it with an empty cell, which is the quickest way to normalize a column of junk markers into real nulls. *Stop and report* refuses the whole run and tells you the row number, the column name and the offending value, which is what you want when the column is supposed to be clean and you would rather know than find out later. The audit report's `non_numeric` count tells you which situation you are in before you commit to a policy.

</details>

<details>
<summary>How do I get accounting-style negatives, like ($1,234.00)?</summary>

Set **Sign style** to accounting parentheses, **Digit grouping** to thousands and **Prefix** to `$`. Negatives come out as `($1,234.00)` and positives as `$1,234.00` — the prefix sits inside both the parentheses and the minus sign, so the plain minus style gives `-$1,234.00` rather than the wrong `$-1,234.00`.

The related trick for a column that has to line up in a fixed-width report is **Sign style: space**, which puts a single blank where a `+` would go. Every value then starts at the same offset whether it is positive or negative. And **no sign at all** writes the magnitude, which is occasionally what a "credits" column next to a "debits" column actually wants.

</details>

<details>
<summary>Can I apply different formats to different columns in one pass?</summary>

Not in one run — one uniform format per run is the entire idea, and it is what makes the result predictable. Chain runs instead: format `price` with two decimals and a `$` prefix, paste the output back in, then format `weight` with three decimals and a ` kg` suffix. Each pass only touches the columns you name, so the earlier result is carried through untouched.

If several columns *do* share a format, name them all at once — **Columns** accepts a mix of names, 1-based indices and ranges, so `price,cost,7-9` is a single pass.

</details>

<details>
<summary>What does a negative number of decimal places do?</summary>

It rounds to the left of the decimal point and zero-fills. `-1` rounds to the nearest ten, `-2` to the nearest hundred, `-3` to the nearest thousand, down to `-9` for billions. `12345` at `-2` becomes `12300`; at `-3` it becomes `12000`.

It is the fastest way to strip false precision out of a column — populations, estimates, budget figures — while keeping the values in the same units, so charts and sums still work without a divide-by-1000 step. If you would rather show the units, use **Notation: compact** instead, which turns `1234567` into `1.23M`. The chosen rounding mode applies either way, so `-2` with truncate gives `12300` while `-2` with ceiling gives `12400`.

</details>
