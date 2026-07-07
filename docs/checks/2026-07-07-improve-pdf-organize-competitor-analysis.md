# pdf-organize — competitor analysis (2026-07-07)

New backlog tool: **pdf-organize** — "Reorder, delete, duplicate, or rotate specific pages of a
PDF in one operation." Built as a chat + CLI tool (file input → PDF output envelope). Like every
other gizza PDF tool (pdf-rotate, pdf-split, pdf-delete-pages, merge-pdf, pdf-compress) it has **no
standalone page**: a page can't fetch an arbitrary PDF and a PDF output has no in-page render mode.

All notes below are **paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Distinct-from-existing check

The repo already ships page-set tools that treat the page spec as an unordered *set* in original
document order:

- **pdf-split** — keep the named pages, drop the rest (order = original).
- **pdf-delete-pages** — drop the named pages, keep the rest (order = original).
- **pdf-rotate** — add a rotation to named pages; order unchanged.

pdf-organize is genuinely distinct because its `order` param is an ordered *sequence*, not a set: a
page can appear zero times (delete), once (keep), or many times (**duplicate**), in **any order**
(**reorder**). Reorder and duplicate are impossible with the existing tools. Doing reorder + delete
+ duplicate + rotate in one pass is the tool's reason to exist. Not a duplicate — built.

## Competitors surveyed (top 5 real tools)

| # | tool | operations offered | selection UX | stated limits |
| - | ---- | ------------------ | ------------ | ------------- |
| 1 | iLovePDF "Organize PDF" | reorder, add/remove pages, "order pages by number", mix | drag-drop thumbnails; "order by number" | not stated on page |
| 2 | pdf.net "Rearrange PDF" | reorder, **delete, duplicate, rotate** | drag-drop thumbnails | 500 MB file cap; no page cap; account required to *download* |
| 3 | PDF2Go "Sort & Delete" | reorder, delete, sort ascending/descending | drag-drop thumbnails; multi-select; reset | none stated |
| 4 | Smallpdf "Organize PDF" | reorder, **duplicate, rotate, delete** (icon per page) | drag-drop thumbnails + per-page icons | free tool |
| 5 | PDFChef "Rearrange" | reorder, delete, rotate | drag-drop thumbnails | free tool |

(Adobe Acrobat online, 10xTools, DonePDF, QwikPDF surfaced too — same feature set; QwikPDF/10xTools
notably run in-browser + private, which is gizza's positioning.)

## Table-stakes → where each lands

| capability | competitors | gizza pdf-organize | tag |
| ---------- | ----------- | ------------------ | --- |
| Reorder pages | all 5 | `order` sequence, e.g. `3,1,2` | in-model ✅ |
| Delete pages | 1,2,3,4,5 | omit from `order`, e.g. `1,3` drops page 2 | in-model ✅ |
| Duplicate pages | 2,4 | repeat in `order`, e.g. `1,1,2` | in-model ✅ |
| Rotate pages | 2,4,5 | `rotate` (±multiple of 90) + `rotate_pages` set | in-model ✅ |
| Sort ascending / descending | 3 | `all` (asc) / `reverse` (desc), or a descending range `4-1` | in-model ✅ |
| Page ranges in selection | implicit | `2-4` inclusive ranges, ascending or descending | in-model ✅ |
| Drag-drop thumbnail editor | all 5 | — no visual page for PDF-output tools | **out-of-model** |
| Live thumbnail preview | all 5 | — no in-page PDF render | **out-of-model** |
| Multi-select + bulk move | 3 | — visual-editor feature | **out-of-model** |
| Merge / mix several PDFs | 1 | separate `merge-pdf` tool; single input here | **out-of-model** |
| Insert blank pages | 1 | different operation, out of scope for "organize existing pages" | **out-of-model** |

Every table-stake capability is either in the descriptor or explicitly listed out-of-model — none
dropped silently. The out-of-model items are all consequences of gizza's browser-local model: PDF
tools take one input and emit a downloadable PDF, so there is no visual thumbnail editor surface.

## UX control patterns

Competitors are uniformly **drag-and-drop visual thumbnail editors**. gizza's equivalent surface for
a PDF-output tool is the text `order` spec (chat + CLI), which is strictly more scriptable/automatable
and is what an LLM can drive directly. The `reverse` keyword and descending ranges cover the "sort"
buttons; repetition covers "duplicate"; omission covers "delete". No visual editor is built (out of
model).

## Positioning notes (paraphrase)

- gizza runs locally / no account, no upload of the final file behind a paywall (pdf.net requires an
  account to download; several gate features). State the 16 MB input cap on the tool (competitors
  quote 500 MB but run server-side).
- One-operation combo (reorder + delete + duplicate + rotate in a single call) matches the strongest
  competitors (Smallpdf, pdf.net) rather than forcing the user to chain split + rotate tools.

## Decisions

- Rotation applies to the **selected original pages before reordering**, so duplicated pages inherit
  the rotation (verified by test + CLI). This is the least-surprising model for "rotate page N then
  arrange" and avoids the "which output position" ambiguity.
- `order` is an ordered sequence (Vec), deliberately unlike split/delete's set semantics.
- Limit: 16 MB input (shared PDF-tool cap). Rotations are metadata (`/Rotate`), non-destructive.
