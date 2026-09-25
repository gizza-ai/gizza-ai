# barcode-batch — competitor analysis (2026-09-24)

Scan run BEFORE implementing. One web search ("bulk barcode generator Code128 EAN UPC from CSV
list batch download zip"), then three reachable competitor pages skimmed. All notes are
paraphrased observations of *functionality*; no competitor copy, branding or trademarks are
reproduced here or in the shipped page. Avery® product codes are cited only as the public
label-geometry designations they are.

## Competitors skimmed

1. **bulkbarcodegenerator.net** — paste / CSV / XLSX upload bulk generator.
2. **unio24.com/tools/bulk-barcode-generator** — client-side bulk generator with label-sheet output.
3. **barcodegen.org/bulk** — minimal paste-a-column generator.

(A fourth hit, issuebadge.com, returned HTTP 410 and was replaced by barcodegen.org per the
"replace unreachable" rule.)

## Observed table stakes

| Capability | bulkbarcode­generator.net | unio24 | barcodegen.org | In gizza model? |
|---|---|---|---|---|
| Paste one value per line | yes | yes | yes | **yes — shipped** |
| CSV / TSV rows (value + label column) | CSV + XLSX upload | CSV `value,label` | first CSV field only | **yes — shipped** (`input_format`, `columns`; XLSX upload is out: no file input on a pure block) |
| Header-row skip | implied | implied | — | **yes — shipped** (`has_header`) |
| Code 128 | yes | yes | yes | **yes — shipped** (auto A/B/C subset pick) |
| EAN-13 / EAN-8 | yes | yes | EAN-13 | **yes — shipped** |
| UPC-A | yes | yes | yes | **yes — shipped** |
| Code 39 | yes | yes | yes | **yes — shipped** |
| Code 93 | — | — | — | **yes — shipped** (free from the encoder) |
| ITF / ITF-14 | yes | yes | yes | **yes — shipped** |
| Codabar | yes | yes | — | **yes — shipped** |
| MSI / Pharmacode | yes | yes | — | **no** — not in the encoder crate; listed, not built |
| QR / DataMatrix / PDF417 | EAN-supp only | yes | yes | **out of scope** — 2D lives in `qr-batch` / `qr-code-generator` |
| Auto symbology per row | — | — | — | **yes — shipped** (`symbology = auto`, a genuine addition) |
| Check-digit auto-calculation | not documented | not documented | not documented | **yes — shipped** (`auto_check_digit`, a genuine addition) |
| PNG output | yes | yes | yes | **yes — shipped** |
| SVG output | yes | — | — | **yes — shipped** (plus `both`) |
| ZIP of all files | yes | yes | batch download | **yes — shipped** |
| Printable label sheet (Avery) | — | Avery PDF + direct print | — | **yes — shipped** (`output = sheet` → PDF, 6 presets) |
| XLSX export of the batch | — | yes | — | **no** — a different tool's job (`csv-to-xlsx`) |
| Foreground / background colour | yes | yes | — | **yes — shipped** (incl. `transparent` background) |
| Bar width / module width | width 2–345 | module width 0.34 mm | scale 2×/3×/4× | **yes — shipped** (`module_width`, 1–10 px) |
| Bar height | 100–250 | 15.0 mm | — | **yes — shipped** (`bar_height`, 20–400 px) |
| Quiet-zone margin | 2–6 | 3.5 mm | — | **yes — shipped** (`quiet_zone`, 0–30 modules) |
| Human-readable text toggle | yes (default on) | yes (default on) | — | **yes — shipped** (`show_text`, default on) |
| HRI font size | 20–36 | caption 3.6 mm | — | **yes — shipped** (`text_size`, 8–48 px) |
| HRI font family / position / alignment | yes | yes | — | **partial** — centred below the symbol only; position/alignment not exposed (documented on the page) |
| Filename prefix | yes | — | — | **yes — shipped** (`name_prefix`) |
| Per-row audit manifest | — | — | — | **yes — shipped** (`include_index` → index.csv, a genuine addition) |
| Batch limit | 20 free / 1000 pro | ~500 | 500–1000 advisory | **500 rows** — stated on the page, no paywall |
| Runs locally, nothing uploaded | — | yes | yes | **yes** — wasm, no network on any surface |

## Defaults adopted (and why)

- `symbology = code128` — the default every scanned competitor leads with.
- `show_text = true`, `format = png`, `output = zip` — the observed common defaults.
- `module_width = 2`, `bar_height = 100`, `quiet_zone = 10`, `text_size = 20` — px equivalents of
  the millimetre defaults unio24 states (module ≈0.34 mm, bar ≈15 mm, margin ≈3.5 mm ≈10 modules).
  10 modules is also the Code 128 / ITF spec minimum quiet zone, so the default is spec-safe.
- `auto_check_digit = true` — every retail symbology here has a mod-10 check digit and pasted
  spreadsheets routinely omit it; silently failing those rows would be the worst default.

## Worked examples put on the page

1. Three SKUs → Code 128 PNG ZIP (`SKU-1001` … ), showing `index.csv` contents.
2. Twelve-digit UPC inputs where the 13th check digit is computed (`03600029145` → `036000291452`).
3. `output = sheet` → an Avery 5160 (30-up US Letter) PDF.

## Out-of-model / deliberately not built

- **XLSX upload** and **XLSX export** — a pure block has no file input; converting is `csv-to-xlsx`'s job.
- **MSI, Pharmacode, Code 11** — MSI/Pharmacode are absent from the pure-Rust encoder; Code 11 is
  present but effectively dead in retail/logistics, so it is not surfaced.
- **2D symbologies** (QR, DataMatrix, PDF417, Aztec) — already covered by `qr-batch`,
  `qr-code-generator`, `qr-styled`; adding them here would collide on search intent.
- **Direct-to-printer** — a browser affordance, not a block capability; the PDF sheet is the
  printable artefact.
- **HRI font family / above-the-symbol placement / letter-spacing** — the PNG renderer uses an
  embedded 8×8 bitmap font (no font file to ship, no font-shaping crate), so alternate typefaces are
  not expressible. Centred-below is the only placement offered, and the page says so.
- **EAN/UPC guard-bar extensions and split HRI digit groups** — bars render at a uniform height with
  the HRI centred underneath. Scannable and what most bulk generators emit; stated as a limit on the
  page rather than silently omitted.
