# timesheet-to-invoice — competitor analysis (2026-08-14)

Scan run BEFORE implementation. All observations are paraphrased from public product pages; no
competitor copy, wording, or branding is reused anywhere in the block.

## Tools reviewed

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | Log My Hours — free invoice generator | Browser form → print/PDF | Timesheet-first workflow; invoice wizard imports tracked hours |
| 2 | Everhour — free invoice generator | Browser form → print/PDF | Time-tracker vendor; template state persists in the browser |
| 3 | oto.work — hourly invoice generator | Browser form → PDF/e-mail | Freelancer-oriented, hourly-billing framing |
| 4 | InvoiceMaker — timesheet invoice template | Downloadable PDF/DOCX/XLSX template | Used as a replacement reference; the fourth candidate (Invoice Simple) returned HTTP 403 and was swapped out |

## Table stakes observed → where each lands

| # | Table stake | Seen on | In/out of model | Landing place |
|---|-------------|---------|-----------------|---------------|
| 1 | Line items with description + quantity/hours + unit rate | 1,2,3,4 | in | `entries` (`description \| hours [\| rate]`), per-line rate override |
| 2 | A single default hourly rate applied to every row | 1,3,4 | in | `rate` (default `100`) |
| 3 | Per-line amount and a running subtotal | 1,2,3,4 | in | computed `amount` per row + `Subtotal` line |
| 4 | Invoice number | 1,2,3 | in | `invoice_number` (default `INV-001`) |
| 5 | Issue date | 1,2,3 | in | `issue_date` (date picker) |
| 6 | Due date, commonly defaulted to issue + 30 days | 1,2,3 | in | `due_date`, else auto-computed from `payment_terms` (default Net 30) |
| 7 | Bill-to / client block | 2,3,4 | in | `client` (multiline) |
| 8 | From / business block | 2,3,4 | in | `business` (multiline) |
| 9 | Tax with a customizable label and percentage | 1,2,3 | in | `tax_rate` + `tax_label` (default `Tax`) |
| 10 | Discount applied BEFORE tax | 1,2 | in | `discount_percent`, applied to the subtotal before tax |
| 11 | Currency symbol selection | 1,2,3 | in | `currency` (free-form symbol/code, default `$`) |
| 12 | Notes / payment-instructions block | 3,4 | in | `notes` (multiline) |
| 13 | Total hours worked stated separately from the money total | 4 | in | `Total hours` line in every format |
| 14 | Dated day-by-day time rows | 4 | in | optional `YYYY-MM-DD` first column on each entry line |
| 15 | Billing-increment rounding of tracked time | timesheet workflow norm (1,4) | in | `round` (0–60 min; 6 and 15 are the common increments) |
| 16 | Reset / start-from-a-preset flow | 1,2,3 | in | `[[example]]` preset chips on the page |
| 17 | Two independent tax lines (Tax 1 / Tax 2) | 1 | out | Single tax line only; documented on the page |
| 18 | Logo upload / visual branding | 1,2,3 | out | Text output has no image channel |
| 19 | PDF / DOCX / XLSX export | 1,2,3,4 | out | This block emits Markdown, plain text, CSV and JSON; rendering to PDF is a separate step |
| 20 | Online payment link (Stripe/PayPal) integration | 2 | out | Needs an account + network; the block is offline and deterministic |
| 21 | Template settings auto-saved in the browser | 2 | out | No persistence layer; deep-link query params carry state instead |
| 22 | E-mail the invoice to the client | 3 | out | No network from the block |
| 23 | Signature lines for employee/supervisor | 4 | out (partially covered) | Free-form `notes` can hold a signature block |

Every table stake above is either a descriptor parameter or an explicit out-of-model row — none
were dropped silently.

## UX control patterns matched

- Date pickers for issue/due dates (`kind = "date"`), not free-text.
- Sliders for the bounded numerics competitors expose as steppers: `payment_terms`, `tax_rate`,
  `discount_percent`, `round`.
- Friendly `<select>` labels for `group_by` and `format` via `[input.labels]`.
- Multiline textareas for the paste-heavy fields (`entries`, `client`, `business`, `notes`) so
  address blocks and pasted timesheets keep their newlines.
- Preset chips (`[[example]]`) standing in for competitors' "start from a template / reset" button:
  a plain hourly invoice, a rounded VAT + discount invoice, and a CSV export.

## Capabilities we add beyond the scanned set

- Flexible hours forms on every row: decimal (`3.5`), `2h 30m`, `2:30`, and real clock ranges
  (`09:00-12:30`, `9am-5pm`) — competitors take a bare number only.
- `group_by = description | date` merges repeated rows into one billed line.
- CSV and JSON output for spreadsheet import and scripting.
- Deterministic, fully offline computation with no account, upload, or tracking.
