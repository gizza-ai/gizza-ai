# Analyze Amazon order-history CSV exports

Paste an Amazon order-history CSV export to calculate total spend, monthly spend, top items, and category totals. The parser handles both classic Amazon Order History Report columns and newer Request My Data / Retail.OrderHistory exports.

Everything runs locally in the browser. Your purchase history is not uploaded.

## Worked example

Input:

```csv
Order Date,Order ID,Title,Category,Quantity,Item Total
2024-01-05,111-1111111-1111111,USB-C Cable,Electronics,2,$9.99
2024-01-20,111-1111111-1111112,Coffee Beans,Grocery,1,$18.50
2024-02-03,111-1111111-1111113,USB-C Cable,Electronics,1,$9.99
```

With `output=summary` and `top=5`, the result reports total spend, a month-by-month table, the repeated USB-C Cable grouped as a top item, and an Electronics/Grocery category breakdown.

## Supported columns

The analyzer detects common columns case-insensitively:

- Date: `Order Date`, `Order Date/Time`, `Date`, `Shipment Date`, `Ship Date`
- Item: `Title`, `Product Name`, `Item Name`, `Name`
- Amount: `Item Total`, `Total Owed`, `Item Subtotal`, `Total Charged`, `Subtotal`, or `Purchase Price Per Unit`
- Optional: `Quantity`, `Order ID`, `Category`

Currency symbols and thousands separators are tolerated. If the export uses per-unit prices, quantity is multiplied into item spend.

## Limits and edge cases

This tool is deterministic CSV aggregation, not a budget app. It does not connect to Amazon, fetch receipts, infer categories with machine learning, or reconcile refunds that are not represented in the CSV rows. Rows with missing or unparsable amounts are counted as skipped in the summary.

<details>
<summary>Where do I get the CSV?</summary>

Use Amazon's order history or Request My Data export flow, then paste the CSV text here. Column names vary by account and export type; the tool recognizes the common variants listed above.
</details>

<details>
<summary>Does the category breakdown always appear?</summary>

Only when the CSV includes a `Category` column. Newer exports often omit categories, so the summary will say that no category column was present.
</details>

<details>
<summary>Can it handle non-dollar currencies?</summary>

Yes for display: common currency symbols such as `$`, `£`, `€`, `¥`, and `₹` are preserved from the first amount seen. The math assumes all rows are in the same currency and does not perform exchange-rate conversion.
</details>

<details>
<summary>Why do some rows show as skipped?</summary>

A row is skipped when it does not contain a parseable amount. This usually means the export has a different amount column name, a blank row, or a status-only line rather than a purchased item.
</details>
