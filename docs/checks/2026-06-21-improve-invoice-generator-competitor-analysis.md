# invoice-generator — competitor analysis & differentiation

**Tool:** `gizza-ai/invoice-generator` — turn line items into a formatted,
printable PDF invoice.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Invoice SaaS (Invoice-Generator.com, Wave, Zoho) | Web | Full-featured, but require an account/upload of your client+billing data to a server; many gate PDF export or add branding. |
| Word/Google Docs templates | App | Manual layout fiddling; totals computed by hand. |
| LaTeX / `wkhtmltopdf` pipelines | DIY | Powerful but a toolchain to install and template to write. |
| Spreadsheet + "export PDF" | App | Manual, and layout/totals are DIY. |

## How gizza's tool is better / different

1. **Local — client + billing data never uploaded.** Generated in WASM (chat SW
   + CLI) with `lopdf` and the base-14 PDF fonts (no font files, no network).
2. **One simple input → a finished PDF.** Line items as
   `description | quantity | unit_price` (one per line); it computes **subtotal,
   tax, and total** and lays out a clean one-page invoice with From/Bill-To,
   invoice number, and date.
3. **No account, no branding, no export paywall.** It's your invoice.
4. **Chat-native.** "Make me an invoice for 10 hours at $75 plus hosting $120,
   8.5% tax" → a downloadable PDF. Also scriptable via the CLI.
5. **Currency + notes** options; tax computed from a percentage.

## Verification

Core unit tests parse line items (incl. stripping `$`/commas), reject malformed
lines, and **generate a PDF that re-loads via lopdf as a valid 1-page document**.
End-to-end CLI produced `invoice-INV-001.pdf` — a valid 1.8 KB `%PDF-` for a
2-item invoice with seller/client/number/date/8.5% tax/notes.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (structured input + PDF file output; same
  no-page pattern as `merge-pdf` / `images-to-pdf`).
- One page, Helvetica, left-aligned columns (amounts not right-aligned, since the
  base-14 path has no glyph-width metrics). Logos/multi-page/templates are out of
  scope for v1.

## Possible future enhancements

- Right-aligned numeric columns (needs font metrics) and a logo image.
- Multi-page overflow for long item lists.
- Due-date / payment-terms fields and a currency-code (ISO) option.
