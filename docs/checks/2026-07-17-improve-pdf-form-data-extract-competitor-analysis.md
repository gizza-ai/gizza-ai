# pdf-form-data-extract — competitor analysis (2026-07-17)

Tool: reads the filled values of an AcroForm PDF and outputs the field name/value
pairs as JSON or CSV. Type: pure-Rust file-input (`lopdf`), chat + CLI (no page —
a binary PDF input with delimited-text/JSON output fits neither the pure-text nor
the ffmpeg file→media page shape, same as `pdf-extract-text` / `pdf-table-extract`).

## Scan (paraphrased — no competitor copy/branding reproduced)

One WebSearch ("extract PDF AcroForm form field values to JSON or CSV online
tool"). Skimmed the top real tools/libraries:

1. **AcroField Inspector** (acrofieldinspector.pro) — a free browser tool that
   inspects a fillable PDF and returns a structured field map: field **names**,
   **types**, validation rules, and **default values**, exported as **JSON** and
   **CSV**, plus a visual preview and field **coordinates**. (Landing page is a JS
   shell; capabilities read from its search snippet.)
2. **pdfFiller — Extract Fillable Fields** (extract-fillable-fields.pdffiller.com)
   — template-driven extraction of filled form data into a table, exported to
   **Excel/CSV**. Workflow-oriented (draw+name regions, bulk upload); does not
   document per-field type metadata.
3. **Aspose.PDF (.NET/Java/Python)** — library; iterates `document.Form`, reading
   each field's **PartialName** and **Value**; exports to **XML / FDF / XFDF /
   JSON**. The canonical "name + value" extraction shape.
4. **Elysia Tools — PDF AcroForm** — recognizes four field categories: **text**,
   **checkbox** (true/false/yes/no), **radio group**, and **dropdown/option
   list**; notes pure **XFA-only** forms are unsupported (AcroForm required).
5. **Adobe Acrobat** — native "Manage Form Data → Merge Data Files into
   Spreadsheet" exports form data to **CSV**.

## Table-stakes → decisions

| Capability | Decision |
|---|---|
| Field **name** (fully-qualified, dotted for nested subforms) | **in-model** — walk `/Fields`→`/Kids`, join partial `/T` names with `.` |
| Field **value** (`/V`) | **in-model** — decode text (incl. UTF-16BE BOM), button on-state names, and multi-select arrays |
| Field **type** (text / checkbox / radio / pushbutton / dropdown / listbox / signature) | **in-model** — from `/FT` + `/Ff` flag bits (Radio/Pushbutton/Combo), inherited from ancestors |
| Output as **JSON** and **CSV** | **in-model** — `format` enum (json default); array of `{name,type,value}` / `name,type,value` rows |
| CSV **delimiter** (comma/semicolon/tab) | **in-model** — `delimiter` enum, mirrors `pdf-table-extract` (`.csv`/`.tsv`) |
| Include vs hide **empty/unfilled** fields | **in-model** — `include_empty` boolean (default true = full field map; false = filled only) |
| UTF-16BE field names (IRS-style forms) | **in-model** — BOM-aware decode |
| **XFA-only** forms | **out-of-model** — needs an XFA/XML form parser; AcroForm (incl. hybrid AcroForm fallback) only, error clearly otherwise |
| Field **coordinates / page location** | **out-of-model** — needs per-widget `/Rect` + page geometry resolution; not part of name/value extraction |
| **Default value** (`/DV`) separate from current `/V` | **out-of-model** — this tool reports the current filled value; `/DV` reporting is a possible future column |
| Choice **option lists** (`/Opt`) / validation rules | **out-of-model** — metadata beyond the filled value; future enrichment |
| Excel (`.xlsx`) export | **out-of-model here** — CSV/TSV covers spreadsheet import; xlsx would need `rust_xlsxwriter` + a binary-download surface |
| Bulk multi-file / template mapping | **out-of-model** — single-document tool by design |

No competitor copy, branding, or trademarks reproduced; out-of-model items are
listed, not built.
