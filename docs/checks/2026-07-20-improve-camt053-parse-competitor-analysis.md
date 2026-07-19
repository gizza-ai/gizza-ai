# camt053-parse — competitor analysis (2026-07-20)

New-tool build scan (done BEFORE implementing). Paraphrased notes only — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed (top 3 reachable)

1. **CAMT Viewer (kibervarnost.si/camt-viewer/)** — free client-side viewer/converter for
   camt.053 AND camt.052 XML. Upload one or more statements, see account summary +
   transactions, export CSV/Excel, generate a cashflow report with charts. Privacy stance:
   everything client-side, nothing uploaded.
2. **BankXLSX (bankxlsx.com, camt.053→CSV/Excel guide)** — SaaS converter; its guide is the
   clearest statement of table-stakes columns: account IBAN + currency, statement id +
   creation date, booking date AND value date, amount + currency, credit/debit indicator,
   counterparty name/IBAN/BIC, references (EndToEndId, InstructionId, bank reference),
   remittance info (Ustrd + parsed Strd), charges, exchange rate, and balances
   (OPBD/CLBD/OPAV/CLAV). Pitfalls it calls out: one `Ntry` can hide dozens of payments
   (batch `NtryDtls/TxDtls` — roll-up vs expand), namespaces + optional nodes break naive
   converters, CdtDbtInd→signed-amount conversion, booking-vs-value date choice, and
   debtor/creditor role swap relative to the direction.
3. **JoeggiCH/camt.053-to-CSV (GitHub, Python + XSL)** — open-source camt.053.001.04 → CSV
   for spreadsheet analysis; single flat transaction table.

(Easy Data Transform's camt.053 page was scanned but replaced as a comparator: it is a
desktop ETL product page with no field-level detail.)

## Table stakes → in-model / out-of-model

| Capability (table stake) | Tag | Decision |
| --- | --- | --- |
| Paste camt.053 XML, any version (.001.02 → .001.13), namespace-agnostic | in-model | parse by LOCAL element names; never bind to one namespace string |
| Accept camt.052 (Rpt) and camt.054 (Ntfctn) siblings — same structure | in-model | CAMT Viewer does 052; `Stmt`/`Rpt`/`Ntfctn` handled alike, `message_type` reported |
| Booking date AND value date per entry | in-model | both extracted (`Dt` or date part of `DtTm`) |
| Amount + currency, CdtDbtInd, entry status (BOOK/PDNG incl. v8+ `<Sts><Cd>`) | in-model | |
| Signed amounts from CdtDbtInd (DBIT negative) | in-model | `signed_amounts` boolean, default true (family invariant with mt940-statement-parse) |
| References: EndToEndId, AcctSvcrRef (bank ref), entry NtryRef | in-model | |
| Bank transaction code (Domn/Fmly/Cd/SubFmlyCd, or Prtry) | in-model | dotted `DOMN.FMLY.SUBFMLY` form, proprietary fallback |
| Counterparty name + IBAN, role-swapped by direction (DBIT→creditor, CRDT→debtor) | in-model | single Counterparty column in CSV; JSON keeps the resolved counterparty |
| Remittance info: Ustrd (joined) + Strd creditor reference | in-model | Strd `CdtrRefInf/Ref` used when Ustrd absent |
| Balances OPBD/CLBD/OPAV/CLAV/PRCD/FWAV/ITBD/ITAV with readable labels | in-model | JSON output; opening/closing also surfaced by type code |
| Batch entries: expand `NtryDtls/TxDtls` to one row per payment vs one row per entry | in-model | `expand_details` boolean, default true (expand) |
| Multi-statement files | in-model | JSON array; CSV `Statement` column |
| Output JSON or CSV; CSV delimiter comma/semicolon/tab/pipe | in-model | family invariant (mt940-statement-parse) |
| Date rendering iso/us/eu/raw | in-model | family invariant |
| Reversal flag (RvslInd) | in-model | kept as a field; sign still follows CdtDbtInd |
| Native .xlsx workbook export | out-of-model | CSV opens in Excel/Sheets; the repo's csv-to-xlsx tool covers xlsx |
| Cashflow charts / counterparty analytics report | out-of-model | analytics UI, not parsing; page stays a converter |
| Multi-FILE batch upload | out-of-model | page is single paste; CLI can loop over files |
| Balance tie-out validation / reconciliation checks | out-of-model | accounting workflow, not parsing |
| PDF rendering of the statement | out-of-model | separate tool family |
| Charges (Chrgs) breakdown + exchange rate per tx | partially in-model | total charges amount + exchange rate extracted when present; full per-charge record breakdown deferred |

## UX control patterns observed

- Competitors ship upload + one-click convert; ours is paste + auto-run (page recompute
  model) with `[[example]]` preset chips for JSON, CSV, and a batch-entry sample.
- Enum params render as `<select>` with friendly `[input.labels]`; booleans as checkboxes
  (signed amounts, expand details).
- Privacy line ("runs in your browser, nothing uploaded") is table-stakes copy for bank-data
  tools — stated generically on the page.

## Design outcome

Params: `data` (required, multiline), `output` (json|csv), `date_format` (iso|us|eu|raw),
`delimiter` (comma|semicolon|tab|pipe), `signed_amounts` (bool, true),
`expand_details` (bool, true). Mirrors the mt940-statement-parse family so the two bank
statement parsers feel identical; camt-specific additions are `expand_details` (batch
entries have no MT940 equivalent) and status/bank-transaction-code/counterparty columns.
