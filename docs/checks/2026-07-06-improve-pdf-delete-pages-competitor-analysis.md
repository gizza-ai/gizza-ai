# pdf-delete-pages — competitor analysis (2026-07-06)

Tool function: remove specified pages from a PDF and keep the rest (the inverse of
`pdf-split`, which keeps a page selection and drops the rest). Real services ship this
as a distinct "Delete/Remove pages" tool separate from Split and Extract, so it is not
a duplicate of the existing `pdf-split` block.

## Competitors scanned (top real tools, paraphrased — no copy/branding reproduced)

1. **iLovePDF — Remove pages.** Upload a PDF, select pages via thumbnails, or type a
   page range like `1-5, 8, 12-20`, then download the file with those pages removed.
2. **RaptorPDF / Smallpdf-class — Delete PDF Pages.** Thumbnail grid; click pages to
   mark for deletion; supports typing ranges `1-5, 10, 15-20`; preview remaining pages
   before download.
3. **Lumin PDF — Delete Pages.** In-editor page manager: multi-select with Ctrl/Cmd
   click or Shift for ranges, delete, then save.

(Also seen: esofttools PDF Page Remover offering "Page Range", "Multiple Page Ranges",
"First Multiple Pages", "Last Multiple Pages" selection modes.)

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where it lands |
|---|---|---|
| Delete a comma list of individual pages (`2,5`) | IN-MODEL | `pages` param grammar |
| Delete inclusive ranges (`3-7`) + multiple ranges (`1-5, 8, 12-20`) | IN-MODEL | `pages` param grammar |
| Bulk odd / even deletion (drop blank even pages from a duplex scan) | IN-MODEL | `pages` accepts `odd` / `even` (consistent with sibling `pdf-split`) |
| "First N" pages shortcut | IN-MODEL (expressible) | `1-N` range covers it |
| "Last N" pages shortcut | Documented | expressible as explicit page numbers once the total is known; grammar kept identical to `pdf-split` (no negative-index fork between the two sibling tools) |
| Clear error when the selection would delete every page | IN-MODEL | core errors "cannot delete all N pages" |
| Out-of-range / zero / non-numeric page rejected with a message | IN-MODEL | core validates against the real page count |
| Visual thumbnail grid + click-to-select | OUT-OF-MODEL | GUI-only; document-input tools have no standalone page (a page can't fetch/upload an arbitrary PDF here), so this tool is chat + CLI only — same surface constraint as every other gizza PDF tool |
| Live preview of remaining pages before download | OUT-OF-MODEL | same reason (no page surface) |
| Reorder / duplicate pages in the same pass | OUT-OF-SCOPE | that is the separate `pdf-organize` backlog tool, not page deletion |

## Design decisions

- `pages` is a required string spec using the **same grammar as `pdf-split`** (1-based,
  comma list, inclusive ranges, reversed ranges normalized, `odd`/`even`) so users learn
  one page-spec language across the PDF tool family. The only inversion is meaning: here
  the spec is the set of pages to **delete**, and the tool keeps the rest.
- Empty spec is an error ("specify which pages to delete") — unlike `pdf-split` where an
  empty spec means "keep all", here it would be a no-op.
- `all` is rejected ("cannot delete all pages") and any selection covering every page is
  rejected — the output must retain at least one page.
- Surface: `Input::Document` (URL or uploaded `ref`), chat + CLI only, no page.

## Out-of-model list (listed, not built)

- Thumbnail visual page picker and remaining-page preview (GUI).
- Drag-reorder / duplicate pages (covered by the distinct `pdf-organize` backlog tool).
