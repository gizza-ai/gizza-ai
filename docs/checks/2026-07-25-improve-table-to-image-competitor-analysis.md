# table-to-image — competitor analysis (2026-07-25)

Tool: **table-to-image** — render a CSV/JSON table as a clean, styled image (header
styling + zebra rows) for sharing. Type: **pure** (deterministic SVG string; no I/O).

## Competitors scanned (paraphrased; no copied copy/branding)

1. **TableConvert — CSV/JSON to PNG/JPEG** (tableconvert.com/csv-to-png, /json-to-png,
   /png-generator). Paste/upload CSV or JSON (recognises object arrays and nested
   structures), edit in a live table editor (sort, dedupe, transpose, regex replace),
   then download a PNG/JPEG. Offers multiple theme colour schemes, transparent
   backgrounds, adaptive layout, and "text clarity" tuning. Runs client-side.
2. **CyberChef-style "Table Text to Image"** (cyberchef.dev/table-text-to-image).
   Converts CSV/Excel/Markdown table text to PNG. Controls for font size, cell padding,
   colours, borders, and whether the first row is a styled header.
3. **Table to Image Converter (browser extension) / Pictify.io / tabletoimage.pics.**
   Turn an HTML/DOM table into a high-resolution PNG. Emphasise alternating (striped)
   row colours, custom fonts, borders, padding, and brand-matched colours; some embed
   the source table data in the PNG metadata.

## Table-stakes parameters → decision

| Capability | Competitors | Our decision |
|---|---|---|
| CSV input | all | **in-model** — `input_format` auto/csv/json, `delimiter` |
| JSON input (array of objects / array of arrays) | TableConvert | **in-model** — objects → keys as headers; arrays → rows |
| First row as styled header | all | **in-model** — `header` boolean, always styled header band |
| Zebra / striped alternating rows | all | **in-model** — `zebra` boolean (default on) |
| Theme colour schemes (light/dark/etc.) | all | **in-model** — `theme` enum: light, dark, slate, blue, green, minimal |
| Accent / header colour | Pictify, TableConvert | **in-model** — `accent` colour param (header band + rule colour) |
| Font size | CyberChef, TableConvert | **in-model** — `font_size` (slider) |
| Cell padding | CyberChef | **in-model** — `cell_padding` (slider) |
| Title / caption above table | generators | **in-model** — `title` text (optional) |
| Text alignment | generators | **in-model** — `align` enum: left/center/right |
| Transparent background | TableConvert, extensions | **in-model** — `theme = "minimal"` uses no page fill; opaque themes fill |
| PNG / JPEG raster output | all | **out-of-model** — we emit a scalable **SVG** (crisper, smaller, embeddable); browsers/`rsvg`/design tools rasterise it to PNG/JPEG in one step. Bundling a wasm raster encoder is not justified for a sharing image and is not in the verified-crate list. Stated as a page limit. |
| Live table editor (sort/dedupe/regex/transpose) | TableConvert | **out-of-model** — separate gizza CSV tools already cover sort/dedupe/pivot/query; this tool only renders. |
| Embed source data in image metadata | tabletoimage.pics | **out-of-model** — niche; SVG stays plain/clean. |
| Custom/brand embedded fonts | Pictify | **out-of-model** — SVG uses generic system font families (`-apple-system, Segoe UI, …`); font embedding bloats output. |
| Nested / deeply-structured JSON | TableConvert | **out-of-model** — flat tabular JSON only (array of flat objects or array of arrays); nested values are stringified. |

## Preset chips (competitors ship theme presets)

`[[example]]` chips prefill a worked table with different themes (Light + zebra, Dark
dashboard, Minimal borderless) so the theme options are one click away.

## Output & verification

Deterministic SVG string → `format = "text"` page (same model as `csv-chart-generator`),
CLI exact-output testable, and page-verifiable. No competitor copy, branding, or
trademarks reproduced.
