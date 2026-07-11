# Competitor analysis — mt940-statement-parse (2026-07-10)

Function: parse a SWIFT **MT940** bank statement (customer statement message) — the
tagged-field format (`:20: :25: :28C: :60F: :61: :86: :62F: :64:`) — into opening/closing
balances and individual transaction lines, and export as CSV or JSON. Runs fully in-browser.

## Top real competitor tools scanned (paraphrased — no copy/branding reused)

1. **statementextract.com — MT940 to Excel** — drag-drop `.sta/.mt940/.940/.txt`, converts a
   SWIFT statement to a spreadsheet. Table-stakes: accept the common MT940 file extensions;
   pull out per-transaction date/amount/description; produce a downloadable tabular export.
2. **alienfusiongenerator.com — MT940 to CSV** — "fast, free, private, in-browser" MT940→CSV
   with **live validation** and export. Table-stakes: clean CSV output; client-side only
   (privacy angle); surface parse errors rather than silently dropping lines.
3. **kibervarnost.si / financialdatatools.com — MT940 viewer** — parses MT940 from "any bank
   worldwide", shows parsed columns (booking date, value date, D/C, amount, transaction type,
   customer reference, bank reference, narrative), a **file-metadata / summary panel** (account
   id, currency, transaction count, closing balance), and exports **CSV, JSON**. Table-stakes:
   both value + entry date; the D/C mark; the 4-char transaction-type code; customer + bank
   reference split on `//`; the `:86:` narrative; a balance summary; multiple export formats.

(Other hits — mt940tocsv.com, propersoft, python `mt940` lib, SharpMt940Lib — reinforce the
same feature set: CSV/JSON/Excel export, balance summary, per-field breakdown.)

## Table-stakes → decision (every one lands in the descriptor or is listed out-of-model)

| Table-stake | Decision |
| --- | --- |
| Per-transaction value date + entry date | **in-model** — both parsed from `:61:` |
| Debit/Credit mark (C/D/RC/RD) + signed amount | **in-model** — `mark` column + `signed_amounts` toggle |
| 4-char transaction-type code (NTRF/NMSC/…) | **in-model** — `Transaction Type` column |
| Customer ref + bank ref (split on `//`) | **in-model** — two columns |
| `:86:` narrative (multi-line, attached to its `:61:`) | **in-model** — `Description` column / JSON field |
| Opening/closing balance (`:60F:`/`:62F:`) with currency | **in-model** — JSON `opening_balance`/`closing_balance`; CSV notes them |
| Available balance `:64:` / forward `:65:` | **in-model** — JSON when present |
| CSV export | **in-model** — `output = csv`, `delimiter` choice |
| JSON export (structured statements) | **in-model** — `output = json` (default) |
| Date reformatting (iso/us/eu/raw) | **in-model** — `date_format` |
| Multi-statement files (several `:20:` blocks) | **in-model** — each becomes a statement; CSV gets a `Statement` column |
| Live validation / clear parse errors | **in-model** — actionable error messages |
| Excel (.xlsx) export | **out-of-model as a native surface** — CSV imports into Excel directly; a binary .xlsx is not needed for parity (JSON+CSV cover downstream import). Listed, not built. |
| Drag-drop file upload of `.sta/.mt940` | out-of-model UX detail — the page takes pasted text (privacy-equivalent, no upload); file-picker paste is a platform concern, not this tool's schema. |
| Cloud/account history, saved conversions | out-of-model — needs a backend/login; gizza is browser-local, no account. |

## Descriptor shipped
`data` (required MT940 text), `output` (json|csv, default json), `date_format`
(iso|us|eu|raw), `delimiter` (comma|semicolon|tab|pipe — CSV only), `signed_amounts`
(bool, default true — debits negative). Original copy/design throughout.
