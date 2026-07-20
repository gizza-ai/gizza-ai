## Read an ISO 20022 CAMT bank statement without an ETL pipeline

Paste a **camt.053** bank statement — the ISO 20022 *Bank-to-Customer Statement* XML your
bank exports — and this tool turns it into structured **JSON** or a flat **CSV**
transaction table. The sibling **camt.052** (account report) and **camt.054**
(debit/credit notification) messages use the same structure and are accepted too.
Everything runs locally with WebAssembly, so **your statement never leaves your browser**.

It reads the fields banks actually send:

- **`Stmt` header** — statement id, sequence number, creation date, and the account's
  **IBAN**, currency and owner name.
- **`Bal` balances** — opening/closing booked (**OPBD**/**CLBD**), available
  (**OPAV**/**CLAV**), interim and forward balances, each with its date, currency and
  CRDT/DBIT sign, plus a readable label.
- **`Ntry` entries** — **booking date** and **value date**, the **CRDT/DBIT** direction,
  amount and currency, status (`BOOK`, `PDNG`), the reversal flag, and the **bank
  transaction code** (`PMNT.ICDT.ESCT`-style domain codes or proprietary ones).
- **`TxDtls` transaction details** — the **end-to-end reference**, bank reference, the
  **counterparty name and IBAN** (creditor for money out, debtor for money in), the
  **remittance info** (`Ustrd` text or the structured creditor reference), and — when
  present — total charges and the FX rate.

Batch entries are handled the way treasurers expect: one `Ntry` that rolls up several
`TxDtls` payments becomes **one row per payment** (each with its own amount and
counterparty), or one summary row with a `details_count` if you switch expansion off.
Multi-statement files produce a JSON array; in CSV each row carries its `Statement` number.

### Worked example

This statement:

```xml
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.02">
 <BkToCstmrStmt>
  <Stmt>
   <Id>STMT-2024-001</Id>
   <Acct><Id><IBAN>NL91ABNA0417164300</IBAN></Id><Ccy>EUR</Ccy></Acct>
   <Ntry><Amt Ccy="EUR">150.50</Amt><CdtDbtInd>DBIT</CdtDbtInd><Sts>BOOK</Sts>
    <BookgDt><Dt>2024-01-02</Dt></BookgDt><ValDt><Dt>2024-01-02</Dt></ValDt>
    <AcctSvcrRef>BANKREF1</AcctSvcrRef>
    <NtryDtls><TxDtls>
     <Refs><EndToEndId>E2E-42</EndToEndId></Refs>
     <RltdPties><Cdtr><Nm>Acme Corp</Nm></Cdtr></RltdPties>
     <RmtInf><Ustrd>Payment invoice 42</Ustrd></RmtInf>
    </TxDtls></NtryDtls></Ntry>
   <Ntry><Amt Ccy="EUR">2000.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Sts>BOOK</Sts>
    <BookgDt><Dt>2024-01-03</Dt></BookgDt><ValDt><Dt>2024-01-03</Dt></ValDt>
    <AcctSvcrRef>BANKREF2</AcctSvcrRef>
    <NtryDtls><TxDtls>
     <Refs><EndToEndId>PAYROLL-MAR</EndToEndId></Refs>
     <RltdPties><Dbtr><Nm>Payroll Ltd</Nm></Dbtr></RltdPties>
     <RmtInf><Ustrd>Salary March</Ustrd></RmtInf>
    </TxDtls></NtryDtls></Ntry>
  </Stmt>
 </BkToCstmrStmt>
</Document>
```

as **CSV** becomes:

```
Statement,Booking Date,Value Date,D/C,Amount,Currency,Status,Bank Transaction Code,End To End Id,Bank Reference,Counterparty,Counterparty IBAN,Description
1,2024-01-02,2024-01-02,DBIT,-150.50,EUR,BOOK,,E2E-42,BANKREF1,Acme Corp,,Payment invoice 42
1,2024-01-03,2024-01-03,CRDT,2000.00,EUR,BOOK,,PAYROLL-MAR,BANKREF2,Payroll Ltd,,Salary March
```

The debit is signed negative and its counterparty is the **creditor** (Acme Corp); the
credit stays positive and its counterparty is the **debtor** (Payroll Ltd). Switch to
**JSON** to also get the account header and every balance as structured objects.

### FAQ

<details>
<summary>What is a camt.053 file?</summary>

camt.053 is the ISO 20022 **Bank-to-Customer Statement** — the XML end-of-day bank
statement that replaces the older SWIFT MT940 format. It nests each transaction inside
`Stmt → Ntry → NtryDtls → TxDtls` elements, which is why it needs parsing before it opens
cleanly in a spreadsheet.

</details>

<details>
<summary>Is my bank data uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely in your browser — the
statement XML is never sent to a server, logged, or stored.

</details>

<details>
<summary>Which camt versions and messages are supported?</summary>

All camt.053 schema versions (`camt.053.001.02` through `.001.13`) — the parser matches
elements by local name and ignores the namespace, and it understands both the old and new
spellings (`<Sts>BOOK</Sts>` vs `<Sts><Cd>BOOK</Cd></Sts>`, `<Cdtr><Nm>` vs
`<Cdtr><Pty><Nm>`). The sibling **camt.052** account report (`Rpt`) and **camt.054**
notification (`Ntfctn`) messages are parsed the same way, and the output notes which
message type the file contained.

</details>

<details>
<summary>Why does one entry become several rows?</summary>

Banks often book a batch (one SEPA collection, one payroll run) as a single `Ntry` whose
`NtryDtls` holds many `TxDtls` payments. With **Expand batch entries** on (the default)
you get one row per payment, each with its own amount, reference and counterparty. Turn it
off to get one row per entry with the batch total and a `details_count` field instead.

</details>

<details>
<summary>What's the difference between the booking date and the value date?</summary>

The **booking date** (`BookgDt`) is when the bank recorded the entry; the **value date**
(`ValDt`) is when the money starts or stops earning interest — the one that drives your
cash position. Both are extracted so you never have to pick at export time.

</details>

<details>
<summary>Who is the counterparty for each row?</summary>

It follows the money: for a **DBIT** (money out) the counterparty is the **creditor** you
paid; for a **CRDT** (money in) it is the **debtor** who paid you. The name comes from
`RltdPties` and the IBAN from the matching `CdtrAcct`/`DbtrAcct`, when the bank includes
them.

</details>

<details>
<summary>Can I get an Excel file?</summary>

Choose **CSV** and open it directly in Excel, Google Sheets or Numbers — a CSV imports as
columns with no extra step. Pick the delimiter your locale expects (comma or semicolon) so
the columns split correctly.

</details>

### Limits & edge cases

- Amounts use the ISO dot decimal (`1234.56`) and are re-emitted with two decimal places
  in CSV. With **Sign amounts** on (the default) DBIT amounts and balances are negative.
- Balances, account header, charges and FX rates live in the **JSON** output; the CSV is
  a flat transaction table (with a `Statement` column) so it imports cleanly into a sheet.
- The parser reads by element name and does **not** validate against the XSD — a
  well-formed file with missing optional tags parses fine, while malformed XML or a
  non-CAMT document reports a clear error instead of failing silently.
- `DtTm` timestamps are trimmed to their date part unless you pick the **Raw** date
  format, which keeps source strings verbatim.
- Group-header (`GrpHdr`) totals and supplementary data (`SplmtryData`) are ignored;
  every `Stmt`/`Rpt`/`Ntfctn` block in the file is parsed.
