# docx-comment-extractor competitor analysis (2026-08-07)

## Scope

Tool: `docx-comment-extractor` — extract tracked review comments from a Microsoft Word `.docx` file into a spreadsheet-ready table with author, anchored document text, date/time, thread parent, status, and comment body.

## Sources checked

- WiseChecker article/tool workflow for exporting Word comments to CSV for external review tracking.
- `ayush-vibrant/docx-comments-extractor` GitHub project description for a script that extracts DOCX comments, associated text, authors and timestamps in multiple formats.
- `taavip/extraxt_docx_comments` GitHub project description for extracting Word comments to an Excel workbook with `python-docx`/XML parsing.
- `docx-comment.app` public tool description for extracting and embedding comments into copyable text.
- General Word/VBA/export-comment workflows surfaced in search results for CSV/text exports.

## Table-stakes capabilities

| Capability | Seen in competitors | In model? | Decision |
| --- | --- | --- | --- |
| Read `.docx` review comments without requiring Microsoft Word | Online tools and scripts parse the Office Open XML container directly | Yes | Pure Rust ZIP/XML parser reads `word/comments.xml` and rejects non-DOCX ZIPs clearly. |
| Include the comment author | CSV/export workflows preserve reviewer names | Yes | `author` is a default column and `authors` can filter by case-insensitive substring. |
| Include comment date/time | Review tracking exports commonly keep timestamps | Yes | Raw ISO `timestamp` is available; default columns split it into `date` and `time`. |
| Include the commented/anchored text | Comment extraction scripts advertise associated/anchored text | Yes | The parser scans comment ranges/references in document body, headers, footers and notes, and emits `anchor`. |
| Include the comment body | All competitors export the reviewer note itself | Yes | `comment` is a default column, with optional newline flattening for one-row-per-comment CSVs. |
| Preserve thread/reply structure | Review workflows need to distinguish replies from top-level issues | Yes | `parent_id` is a default column; `include_replies` can hide replies. |
| Show open vs resolved comments | Word's review state matters for tracking | Yes | `commentsExtended.xml` is parsed where present; `status` filters `all`, `open`, or `resolved`. |
| Output CSV/spreadsheet formats | CSV/XLSX/text export is common | Yes | `format` supports `csv`, `tsv`, `json`, and `markdown`. XLSX writing is not needed because CSV/TSV import cleanly into spreadsheets. |
| Choose exported columns | Scripts often expose optional details such as initials/timestamps | Yes | `columns` accepts any ordered subset of the canonical fields. |
| Direct browser drag-and-drop upload UI | Online tools accept local DOCX uploads | Out of model for this block | The block is a chat/CLI document-input tool using `url` or uploaded `ref`; no generic binary page surface exists here. |
| Edit or embed comments back into DOCX | Some tools mention embedding comments into text or workflows | Out of model | This tool is extraction-only and never mutates/repackages the document. |
| Export native `.xlsx` workbooks | Some scripts write Excel files | Out of model for first version | Text table outputs keep the wasm/tool surface simple; users can import CSV/TSV into spreadsheet software. |
| Legacy `.doc` support | Word users may still have old binary files | Out of model | The parser validates modern `.docx` ZIP/XML only and reports `.doc` as unsupported. |

## Defaults and UX choices

- Default output is `csv`, matching review-tracking workflows and spreadsheet import.
- Default columns are `id,parent_id,author,date,time,status,anchor,comment`, balancing triage usefulness with compact output; `initials` and raw `timestamp` are opt-in.
- Default `status=all` and `include_replies=true` avoid silently hiding review content.
- Default `flatten_newlines=true` keeps CSV/TSV to one physical row per comment; users can disable it when multi-paragraph fidelity is more important than row shape.
- Author filtering uses comma-separated case-insensitive substrings because competitor workflows often focus on a reviewer subset.

## Worked examples to support

1. A DOCX with a top-level comment and a reply should emit two rows with `parent_id` linking the reply to its thread.
2. `format=json&columns=author,comment` should return machine-readable row objects with only those keys.
3. `status=resolved` should return only comments marked resolved in Word's `commentsExtended.xml` part.
4. `include_replies=false` should produce a top-level-review triage table.

## Limits and honesty notes

- Input is capped at 16 MiB by the wasm handler.
- The tool reads review comments only; it does not extract the whole document body (use the document-text extraction tool for that purpose).
- Anchors depend on Word comment-range markup. Orphaned comments with no range/reference still appear, but their `anchor` is blank.
- Resolved/open state depends on the optional `word/commentsExtended.xml` part; when absent, comments are treated as open top-level comments.
- Password-protected/corrupt DOCX files and legacy `.doc` binaries are rejected rather than guessed.
