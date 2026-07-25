# pdf-header-footer — competitor analysis (2026-07-25)

Tool: **pdf-header-footer** — "Adds custom header and footer text to every page of a PDF."
Type: pure (lopdf content-stream overlay, base-14 fonts). Chat + CLI only
(binary-in / PDF-out has no page render mode in this repo's generated page system).

## Competitors scanned (top 3, paraphrased — no copy/branding reused)

1. **OneClickPDF – Add Header/Footer.** Six independent zones (header-left/center/right,
   footer-left/center/right). Smart variables `{page}`, `{pages}`, `{date}`, `{filename}` that
   auto-resolve per page. Applies to all pages.
2. **PDFill – Header and Footer.** Type text plus date/time/title before or after a number; full
   control over font name, font style, font size and font color; positions text in header and
   footer bands.
3. **PDF Candy – Add Header and Footer.** Header + footer text with page numbers, titles, dates or
   custom notes; a few clicks, whole document.

(Also surveyed: Sejda, PDFFooter.com, Drawboard — same feature envelope: header text, footer text,
left/center/right alignment, page-number/date variables, font + colour controls, page range.)

## Table-stakes → decision (every item lands in the descriptor or is listed out-of-model)

| Capability | Fit | Where |
| --- | --- | --- |
| Header text (top of every page) | in-model | `header` param |
| Footer text (bottom of every page) | in-model | `footer` param |
| Left / center / right alignment | in-model | `header_align`, `footer_align` enums (independent) |
| `{page}` current page number token | in-model (deterministic page index) | documented in `header`/`footer` describe; substituted in core |
| `{pages}` total page count token | in-model (deterministic) | same |
| Font family | in-model | `font` enum (helvetica / times / courier — base-14, no embedding) |
| Font size | in-model | `font_size` (4–144 pt) |
| Text colour | in-model | `color` hex |
| Margin from edge | in-model | `margin` (0–400 pt) |
| Opacity (faint/draft look) | in-model | `opacity` (0.05–1.0) |
| Page range (skip cover etc.) | in-model | `pages` spec ("all" / "1,3-5" / "2-") |
| `{date}` auto-resolving date token | **out-of-model** | the pure core has no runtime clock; users type a fixed date string instead. Listed, not built. |
| `{filename}` token | **out-of-model** | the core stamps bytes and does not carry the source filename; users type the label. Listed, not built. |
| Six *simultaneous* independent zones (L+C+R in one band) | **partial / out-of-model** | we expose one aligned text per band (header + footer, each L/C/R). A single band can hold one aligned string, not three at once. Covers the dominant use case (one header line + one footer line). |
| Custom TrueType/OTF font upload | out-of-model | base-14 fonts only (no embedding), keeping output small and backend-portable. |

## Design summary

Params use the same conventions as other document/PDF-style tools where they overlap
(font/size/color/margin/opacity/pages), and add the header/footer-specific `header`,
`footer`, `header_align`, and `footer_align`. At least one of `header`/`footer` must be
non-empty. `{page}`/`{pages}` tokens are substituted per page; anything the base-14
WinAnsi encoding can't represent degrades to `?`.

Sources: OneClickPDF, PDFill, PDF Candy, Sejda, PDFFooter.com, Drawboard (add-header-footer pages).
