## Read a SWIFT MT940 statement without a spreadsheet macro

Paste a **MT940** bank statement — the tagged-field SWIFT format your bank exports as
`.sta`, `.mt940`, `.940` or `.txt` — and this tool turns it into structured **JSON** or a
flat **CSV** transaction table. Everything runs locally with WebAssembly, so **your
statement never leaves your browser**.

It reads the fields banks actually send:

- **`:20:` / `:25:` / `:28C:`** — statement reference, account identification, and statement
  number.
- **`:60F:` / `:62F:` / `:64:` / `:65:`** — opening, closing, available and forward-available
  balances, each with its currency and debit/credit sign.
- **`:61:`** — every statement line: **value date** and **entry date**, the **C/D/RC/RD**
  debit-credit mark, the amount, the 4-character **transaction-type code** (`NTRF`, `NMSC`,
  `NDDT`, …), and the **customer** and **bank references** split on `//`.
- **`:86:`** — the narrative attached to each transaction, kept intact even across multiple
  lines.

Multi-statement files (several `:20:` blocks in one export) are handled — in CSV each row
carries its `Statement` number so nothing is merged by accident.

### Worked example

This statement:

```
:20:REF12345
:25:NL91ABNA0417164300
:28C:00123/001
:60F:C240101EUR1000,00
:61:2401020102D150,50NTRFNONREF//BANKREF1
:86:Payment to Acme Corp invoice 42
:61:2401030103C2000,00NTRFPAYROLL//BANKREF2
:86:Salary March
:62F:C240131EUR2849,50
```

as **CSV** becomes:

```
Statement,Value Date,Entry Date,D/C,Amount,Currency,Transaction Type,Customer Reference,Bank Reference,Description
1,2024-01-02,2024-01-02,D,-150.50,EUR,NTRF,NONREF,BANKREF1,Payment to Acme Corp invoice 42
1,2024-01-03,2024-01-03,C,2000.00,EUR,NTRF,PAYROLL,BANKREF2,Salary March
```

The debit line is signed negative, the credit positive, the `240102` dates are expanded to
ISO, and each `:86:` narrative lands on its own row. Switch to **JSON** to also get the
opening/closing/available balances as structured objects.

### FAQ

<details>
<summary>What is an MT940 file?</summary>

MT940 is the SWIFT **Customer Statement Message** — the standard end-of-day bank statement
format. It is plain text made of colon-delimited tags (`:20:`, `:61:`, `:86:`, …) rather than
columns, which is why it needs parsing before it opens cleanly in a spreadsheet.

</details>

<details>
<summary>Is my bank data uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser — the statement
text is never sent to a server, logged, or stored.

</details>

<details>
<summary>Why are some amounts negative?</summary>

With **Sign amounts** on (the default), a debit (`D`/`RD`) is written as a negative number and
a credit (`C`/`RC`) as positive, so a column of amounts sums to the net movement. Turn the
toggle off to keep every amount positive and rely on the **D/C** column for direction instead.

</details>

<details>
<summary>What's the difference between the value date and the entry date?</summary>

The **value date** is when the money is credited/debited for interest purposes (the 6-digit
`YYMMDD` at the start of `:61:`); the **entry date** is when the bank booked it (the optional
4-digit `MMDD` that follows). When the entry date omits the year, this tool borrows it from the
value date so both render as full dates.

</details>

<details>
<summary>Does it handle files with several statements?</summary>

Yes. Each `:20:` starts a new statement. In JSON you get an array of statement objects; in CSV
every transaction row includes a `Statement` column (1, 2, 3, …) so lines from different
statements stay distinguishable.

</details>

<details>
<summary>Can I get an Excel file?</summary>

Choose **CSV** and open it directly in Excel, Google Sheets or Numbers — a CSV imports as
columns with no extra step. Pick the delimiter your locale expects (comma or semicolon) so the
columns split correctly.

</details>

### Limits & edge cases

- Amounts use the SWIFT comma decimal (`1234,56`); they are parsed and re-emitted with two
  decimal places in CSV. Balances keep their currency from the `:60F:` opening balance.
- Balances live in the **JSON** output; the CSV is a flat transaction table (with a
  `Statement` column) so it imports cleanly into a sheet.
- SWIFT block headers (`{1:…}{4:…}`) and the block-4 `-` trailer are ignored, so you can paste
  either the raw wire message or just the tag lines.
- Unrecognized tags (like `:21:`) are skipped rather than causing an error; a genuinely
  malformed balance or `:61:` line reports a clear message instead of failing silently.
