# pdf-bookmarks competitor analysis (2026-08-09)

## Scope

Tool: `pdf-bookmarks` — add, list, generate, or remove a PDF outline/bookmark tree for easier navigation.

## Sources checked

Web search: `PDF add bookmarks outline online tool PDF bookmarks page navigation`.

Top real tools/pages reviewed from the search result snippets and public product pages:

1. A browser PDF bookmark editor from pdf.imagestool.com.
2. bookmarkpdf.com, a browser-based PDF bookmark creator.
3. ALLYX ONE's PDF bookmark editor.
4. Adobe Acrobat help for the desktop bookmark workflow, used as a baseline for table-stakes behaviour rather than copied UI text.

The extraction backend was unavailable in this environment, so decisions below are based on reachable search metadata plus the common Acrobat/bookmark-editor workflow. No competitor wording, branding, or UI copy is reused.

## Table-stakes capabilities and fit

| Capability | Competitor pattern | In current model? | Decision |
| --- | --- | --- | --- |
| Add a nested outline | Bookmark editors let users create hierarchical sections and subsections. | Yes | Support an indented text format and JSON with `children` for nested bookmarks. |
| Page target per bookmark | Bookmarks point to specific pages. | Yes | Each entry requires a 1-based page number; out-of-range pages are clamped with warnings. |
| Existing outline inspection | Editors often show/import/export the existing outline. | Yes | `mode=list` returns a readable tree; JSON input accepts the same conceptual shape for list-edit-apply workflows. |
| Replace vs append | Some workflows rebuild the whole outline; others add to it. | Yes | `replace=true` by default, `replace=false` appends after existing entries. |
| Remove/clear outline | Bookmark editors expose deletion/cleanup operations. | Yes | `mode=remove` strips the outline tree and bookmark-pane page mode. |
| Per-page auto bookmarks | Some tools offer simple generated bookmarks for every page. | Yes | `mode=per-page` creates one flat entry per page with `{n}` and `{total}` template variables. |
| Open/collapsed nesting | Desktop editors can show nested items expanded/collapsed. | Yes | `expanded=true/false` controls positive/negative PDF `/Count` values. |
| Open bookmark pane | Viewers can be instructed to show the outline panel. | Yes | `show_pane=true` sets `/PageMode /UseOutlines`. |
| Destination zoom | Acrobat-style bookmarks can fit page, fit width, or keep current zoom. | Yes | `zoom=fit`, `fit-width`, or `keep`. |
| Drag-and-drop visual tree editing | Browser competitors provide interactive tree controls. | Out of model | This repo has no generic binary-PDF page/editor surface for PDF-in/PDF-out tools, so the tool is chat + CLI with text/JSON specs. |
| Coordinate/region destinations | Acrobat can target a page location or selected region. | Out of model for first version | The gizza descriptor stays compact and page-number based; full coordinate destinations would require much more PDF viewer UI. |
| Automatic TOC detection/OCR | Some paid/desktop flows can infer bookmarks from headings. | Out of model | Requires document layout/ML/OCR and error-prone heuristics; not included. |
| Browser-only no-upload UX | Several competitors emphasize local browser operation. | Out of model for this tool surface | PDF document input + PDF output has no standalone page pattern here; chat/CLI uses `url` or `ref`. |

## Descriptor / UX choices

- `Input::Document` so chat and CLI accept either `url` or `ref` PDF sources.
- `mode` enum: `list`, `apply`, `per-page`, `remove`.
- `bookmarks` text field for `mode=apply`; accepts either indented lines such as `Chapter 1 | 3` or JSON entries.
- Boolean controls for replace/expanded/show-pane; default to the safer common workflow of replacing with an expanded outline and showing the bookmark pane.
- `zoom` enum matches the useful subset: fit page, fit width, keep current zoom.
- `per_page_label` exposes the simple auto-generation workflow without a complex visual editor.

## Worked examples used for verification

- List a PDF with no outline: expect a `0 bookmarks` response.
- Apply `Intro | 1` and nested entries to a blank PDF, then list it back from core tests.
- Generate per-page labels with `Sheet {n} of {total}`.
- Remove an existing outline and verify the outline tree is empty.

## Limits and edge cases

- Maximum nesting depth: 6 levels.
- Maximum bookmark count: 5000 entries.
- Page numbers are 1-based. Pages above the document count are clamped to the last page with warnings.
- JSON input must be an array of objects or an object with a `bookmarks` array; each item needs a non-empty `title` and a page.
- Colour attributes support `#rgb`, `#rrggbb`, and a small set of common names.
- Encrypted, malformed, or non-PDF bytes are rejected by the PDF parser.
