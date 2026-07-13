# amazon-order-analyzer — competitor analysis (2026-07-13)

Function: parse an Amazon order-history CSV export and summarize total spend by
month, with top items and a category breakdown. Pure, in-browser, no upload.

## Competitor scan (top 3 real tools)

1. **Amalytix — Amazon Order Analysis** (amalytix.com/en/tools/amazon-order-analysis/).
   Upload your order-history file; it is processed **locally in the browser** and
   shows spending patterns. Table-stakes it advertises: local/private processing,
   spend over time, and a breakdown of what you bought. Marketing angle is privacy
   ("all processed locally").
2. **OrderPro Analytics** (orderproanalytics.com/amazon). Actively-maintained
   exporter/analyzer; positions itself as the replacement after Amazon removed the
   native CSV export (March 2023). Advertises **monthly spending trends**, **top
   categories**, and **product / price breakdowns** generated from the exported
   orders. This is the closest feature match to our brief.
3. **Tiller — Amazon order-history import** (tiller.com/how-to-download-your-amazon-order-history-report/).
   Guides you to download the order-history report and import the CSV line items
   into a budgeting spreadsheet (Foundation Template). Table-stakes: ingest the
   report CSV, categorize line items, roll up totals. It is a spreadsheet workflow,
   not a one-shot analyzer.

(Amazon's own Business Analytics offers category spend for *business* accounts
only, behind login — not a general-purpose CSV tool, so not counted as one of the
three consumer tools; noted for context.)

## Table-stakes (each → in-model descriptor param, or out-of-model list)

| Capability | Decision | Where |
| --- | --- | --- |
| Ingest the order-history CSV (quoted titles, both report shapes) | **in-model** | `csv` param; robust header detection |
| Total spend + order/item counts + date range | **in-model** | always in summary/JSON caption |
| Spend by month (monthly trend) | **in-model** | "Spend by month" section |
| Top items by spend | **in-model** | `top` param (default 10); "Top items" section |
| Category breakdown | **in-model** *(when the export has a Category column)* | "Categories" section; gracefully omitted for the newer "Request My Data" export that has no Category column |
| Structured output for scripting | **in-model** | `output` = `summary` \| `json` |
| Currency-symbol / thousands tolerance ($, £, €, commas, "USD ") | **in-model** | amount parser strips symbols/commas |
| Local / private processing (no upload) | **in-model** (inherent) | runs as WASM in the tab; stated on page |
| Google-Sheets / Excel export of the raw rows | **out-of-model** | listed, not built — gizza returns text/JSON; use a CSV tool to reshape rows |
| Live scraping of Amazon (browser extension) | **out-of-model** | gizza is offline compute; Amazon removed native export, user brings the CSV |
| Return/refund reconciliation, per-seller analytics | **out-of-model** | not in the brief; would need refund columns not present in the standard report |

## In/out-of-model summary

- **In-model (built):** CSV ingest with tolerant header + amount parsing, spend by
  month, top items by spend, category breakdown (when present), summary + JSON
  output, configurable top-N. Two Amazon export shapes handled: the classic
  **Order History Report** (`Order Date`, `Title`, `Category`, `Item Total`, …) and
  the **Request My Data / Retail.OrderHistory** export (`Order Date`, `Product Name`,
  `Total Owed`, …).
- **Out-of-model (listed only):** spreadsheet/Sheets export of raw rows, live Amazon
  scraping, refund/return reconciliation, per-seller analytics.

## UX controls / examples ours matches

- Preset **example chips**: a small classic-report CSV and a Request-My-Data CSV.
- `output` renders as a `<select>` (Summary / JSON); `top` is a number field.
- Worked example on the page shows input CSV → exact summary output.
- Privacy note ("runs in your browser, nothing is uploaded") mirrors Amalytix's
  main selling point without copying any wording.

No competitor copy, branding, or trademarks reproduced — decisions paraphrased.
