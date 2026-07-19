# ppt-to-text — competitor analysis (2026-07-17)

Tool: extract readable plain text from a **legacy binary PowerPoint `.ppt` (PowerPoint
97–2003)** presentation. A `.ppt` is an OLE2 / Compound File Binary (CFB) container whose
`PowerPoint Document` stream holds a record tree; text lives in `TextCharsAtom` (UTF-16LE) and
`TextBytesAtom` (Latin-1) atoms inside Slide and Notes containers. This is the classic
`catppt` / Apache-POI-HSLF extraction path. Distinct from the ZIP-based `.pptx` (Office Open
XML) and from every other doc tool in the repo (`doc-to-text` = Word `.doc`, `docx-text-extract`
= `.docx`, `document-text-extract` = pdf/docx/epub, `pdf-extract-text` = PDF).

## Competitors scanned (top real tools)

1. **SlidesPilot — PPT to Text** (slidespilot.com/features/ppt-to-text) — upload `.ppt`/`.pptx`,
   extracts slide text, download a `.txt`. No visible options; upload-and-convert only.
2. **Sharayeh — Extract Text from PPT** (sharayeh.com/en/tools/extract-text-from-ppt) — richest
   feature set: pulls titles, bullets, speaker notes, text boxes, tables, SmartArt; toggles for
   notes on/off and boilerplate removal; regex slide filter; outputs txt/docx/markdown/json;
   Pro tier adds OCR of rasterized image text. States 50 MB (starter) / 500 MB (pro) size caps.
3. **Convertio — PPT to TXT** (convertio.co/ppt-txt) — generic file converter; extracts all
   slide text to a plain `.txt`, no configuration surface.
4. **CloudConvert / Zamzar — PPT→TXT** — same generic-converter shape: upload, convert, download
   `.txt`; no content-type toggles.
5. **AutoSlide / MagicSlides — PPT to Text** — free upload converters; extract slide text
   (titles + bullets + text boxes) to plain text; note both `.ppt` and `.pptx` support.

## Table-stakes → model-fit decisions

| Capability | Fit | Where it lands |
|---|---|---|
| Extract slide text (titles, bullets, text boxes) | **in-model** | core record walk of Slide (`0x03EE`) containers → all `TextCharsAtom`/`TextBytesAtom` |
| Extract table cell text | **in-model** | table cells are ordinary text placeholders in the slide drawing — captured by the same walk |
| Speaker notes + notes on/off toggle | **in-model** | `notes` enum `include`/`exclude`/`only`; notes are tagged by their `Notes` (`0x03F0`) ancestor container |
| Clean vs raw whitespace | **in-model** | `whitespace` enum `clean`/`raw` (collapse blank-line runs + trim, or verbatim) |
| Word / paragraph / slide counts in output | **in-model** | flat JSON response (`words`, `paragraphs`, `text_runs`, `slides`, `truncated`) |
| Plain-text output | **in-model** | JSON `text` field is the extracted plain text |
| Both `.ppt` and `.pptx` in one tool | out-of-scope here | `.pptx` (Office Open XML ZIP) is a separate format; this tool is scoped to the legacy binary `.ppt` only (mirrors how `doc-to-text` is `.doc`-only vs `docx-text-extract`) |

## Out-of-model competitor features (listed, not built)

- **OCR of rasterized image text** (Sharayeh Pro) — needs an OCR/ML model; gizza is pure-Rust +
  ffmpeg with no ML model surface. Out-of-model (same class as the deferred `printed-text-ocr`).
- **DOCX / Markdown export formats** (Sharayeh) — the chat/CLI surface returns structured JSON
  with the plain text; format re-encoding chains to existing tools (e.g. `text-to-pdf`) and is a
  site download-button concern, not a distinct capability here.
- **Boilerplate removal** (auto-strip footers / page numbers / confidentiality notices) —
  heuristic content editing, out of scope for a faithful extractor; would risk dropping real
  content.
- **Regex slide filtering** (extract only slides matching a pattern) — plausibly in-model later
  but not a table-stake across the field (only one competitor ships it); deferred to keep the v1
  descriptor lean. Users can post-filter the returned text.
- **Bulk / multi-file extraction** — an invocation pattern, not a distinct tool surface.
- **Size caps of 50–500 MB** — marketing tiers; we cap input at 16 MiB (repo default) and output
  at 1M chars, stated plainly.

## Surface

Binary file input → plain text output = the **no-page file-input pattern** (like `doc-to-text`,
`docx-text-extract`, `document-text-extract`, `pdf-extract-text`): **chat + CLI only, no web
page** (a binary upload + text result fits neither the pure-text nor the ffmpeg media page
shapes). No competitor copy, branding, or trademarks reproduced.
