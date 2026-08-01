# beancount-to-csv — competitor analysis (2026-07-31)

Tool function: convert common Beancount / Ledger plain-text-accounting journals into a flat CSV of dated postings, and rebuild a simple journal from that CSV for spreadsheet round-tripping.

Distinct from existing tools:
- `blocks/csv-to-ledger` emits ledger-like entries from a simple CSV, but does not parse an existing Beancount/Ledger journal into posting rows and does not provide a bidirectional flat schema.
- CSV utilities (`csv-cleaner`, `csv-query`, `csv-to-table`, etc.) operate on already-tabular data and do not understand transaction headers/postings.
- This tool is viable as a deterministic text parser/formatter; no accounting engine, price lookup, import sync, or network access is required.

## Scan (top competitors, paraphrased — no copy/branding reproduced)

1. **Beancount ecosystem import/export scripts and bean-query examples.** Common workflows flatten journal postings or query results into CSV for spreadsheet analysis. Table-stakes: date, payee/narration, account, units/amount, currency, and cost/price fields; spreadsheet-friendly header row; clear limits around not evaluating the whole accounting ledger.
2. **Ledger / hledger CSV conversion docs.** These tools map CSV rows into journal postings with account, amount, currency, date, and description fields. Table-stakes: configurable dialect, simple CSV input schema, multiple postings per transaction, and support for symbol currencies.
3. **Plain-text-accounting spreadsheet workflows.** Users commonly paste journal snippets into sheets or rebuild simple postings from sheets. Table-stakes: one row per posting, repeated transaction header fields, blank-date continuation rows, and predictable round trips for common transactions.
4. **Accounting CSV importers.** General importers prioritize robust CSV delimiters and visible limits. Table-stakes: comma/semicolon/tab/pipe delimiters, quoted CSV handling, and useful errors for missing columns or invalid amounts.

## Table-stakes → decision

| Table-stake | In/out model | Decision |
|---|---:|---|
| Journal → CSV flattening | in | `direction = to-csv`, one row per posting |
| CSV → journal rebuilding | in | `direction = from-csv` |
| Beancount and Ledger-ish syntax | in | Parse common dated transactions; `journal_format` selects emitted dialect |
| Header fields repeated per posting | in | Columns include `date`, `flag`, `payee`, `narration` |
| Posting account/amount/currency | in | Columns include `account`, `amount`, `currency` |
| Cost / price annotations | in | Carry `{...}` and `@...` through verbatim |
| Comments | in | Carry trailing posting comments in `comment` |
| Delimiter choices | in | `delimiter = comma | semicolon | tab | pipe` |
| Full Beancount evaluation | out | No inventory, balance assertions, interpolation, plugins, includes, or price lookup |
| Complete directive preservation | out | Non-transaction directives are skipped and limits disclose that |
| Account inference / import rules | out | Requires user-specific configuration; separate importer workflow |

## Descriptor / UX decisions

- The page exposes a multiline input textarea, direction select, journal-format select, and delimiter select.
- Preset chips cover a Beancount journal to CSV, a CSV back to Beancount, and a CSV to Ledger output.
- The fixed flat CSV schema is documented in the page copy and examples: `date,flag,payee,narration,account,amount,currency,cost,price,comment`.
- Limits are explicit: useful spreadsheet reshaping, not a full accounting engine.
