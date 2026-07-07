## QIF / OFX / QFX to CSV converter

Paste a bank or accounting export in **QIF** (Quicken Interchange Format) or
**OFX/QFX** (Open Financial Exchange) and get a clean, normalized CSV — the same
columns every time, ready to import into a budgeting app or open in a spreadsheet.
Nothing is uploaded: the conversion runs locally in WebAssembly, so your financial
data never leaves your device.

### What you get

Every transaction becomes one CSV row with a fixed column set:

`Date, Amount, Payee, Memo, Category, Check Number, Type, FITID`

Columns that don't apply to your format stay blank — QIF has no `Type`/`FITID`,
OFX statements usually carry no `Category`. Turn on **Drop empty columns** to
remove any column that's blank for every row (handy for a tidy import), or leave
it off to keep the full 8-column layout so a saved import template always lines up.

### How it works

- **Input format** — leave it on **auto** and the tool detects QIF vs OFX from the
  content; force `qif` or `ofx` if you prefer. `ofx` also handles `.qfx` files and
  both OFX 1.x (SGML) and OFX 2.x (XML).
- **Date column format** — QIF dates are ambiguous (is `03/04` March 4th or April
  3rd?) and OFX dates are `YYYYMMDD`. Pick **ISO** `2010-03-15` (the default, and
  the safest for imports), **US** `MM/DD/YYYY`, **EU** `DD/MM/YYYY`, or **raw** to
  keep the source text untouched. QIF month/day order is auto-detected — US
  month-first unless the first field is greater than 12.
- **Amounts** — thousands separators are stripped (`1,250.00` becomes `1250.00`)
  and the sign is preserved. Toggle **Flip amount signs** if your bank exports
  debits as positive (or your budgeting app expects the opposite convention).
- **Delimiter** — comma (default), semicolon, tab (TSV), or pipe.

### Worked example

This QIF input:

```
!Type:Bank
D03/15/2010
T-50.00
PTarget Store
MWeekly run
LFood:Groceries
N1002
^
```

with **ISO** dates and **Drop empty columns** on becomes:

```
Date,Amount,Payee,Memo,Category,Check Number
2010-03-15,-50.00,Target Store,Weekly run,Food:Groceries,1002
```

### Limits and edge cases

- Convert **one export at a time** — paste a single QIF or OFX file's contents.
- **Split transactions** (QIF `S`/`$` lines) are exported as one row per
  transaction; when the transaction has no top-level category, the split
  categories are joined into the `Category` cell (e.g. `Food; Household`).
- A date that can't be parsed is left as its **raw** source text rather than
  failing the whole file.
- Investment (`!Type:Invst`) QIF blocks and OFX investment statements aren't
  broken out into buy/sell columns — banking/credit-card transactions are the
  supported shape.

<details>
<summary>Is my bank data uploaded anywhere?</summary>

No. The converter is compiled to WebAssembly and runs entirely in your browser
tab. Your QIF/OFX contents are never sent to a server.

</details>

<details>
<summary>What's the difference between QIF, OFX and QFX?</summary>

**QIF** is Quicken's older plain-text format — one line per field with a
single-letter code (`D` date, `T` amount, `P` payee), records separated by `^`.
**OFX** is a tag-based format (SGML in 1.x, XML in 2.x) used by most bank
"download transactions" buttons. **QFX** is Intuit's OFX variant; it parses the
same way here — select `ofx` (or leave it on `auto`).

</details>

<details>
<summary>Why is my Date column empty or unchanged?</summary>

If a date can't be parsed it's kept as the original text so nothing is lost. Try
the **raw** date option to see exactly what the file contained, or switch between
**US** and **EU** if the month and day look swapped — QIF files don't state their
date order, so the tool assumes month-first unless the first number is above 12.

</details>

<details>
<summary>Can I convert several files at once, or export to Excel?</summary>

This tool converts one export at a time to CSV. To combine several files, convert
each and stack the rows; to get a `.xlsx` workbook, run the resulting CSV through
the CSV-to-XLSX tool. Multi-file batch conversion and direct Excel/QBO output need
a server and aren't part of this in-browser tool.

</details>

<details>
<summary>My budgeting app wants expenses as positive numbers — can it do that?</summary>

Yes. Turn on **Flip amount signs** and every amount's sign is reversed, so a
`-50.00` debit becomes `50.00`. Zero amounts are left as-is.

</details>
