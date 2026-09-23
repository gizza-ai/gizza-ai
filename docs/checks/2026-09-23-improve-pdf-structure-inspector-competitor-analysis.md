# pdf-structure-inspector — competitor analysis (2026-09-23)

Scan run **before** implementation, per `/create-next-tool` step 4. Everything below is a
**paraphrased** feature/UX observation. No competitor copy, naming, branding, or trademark text
was reused anywhere in this tool's page, descriptor, or output strings.

## Scope of the tool

Parse a PDF's *file structure* — header version, cross-reference table/stream, trailer
dictionary, the indirect-object tree, and stream dictionaries — **without rendering page
content**. This is the "what is actually in this file" view a PDF developer or forensics analyst
needs when a document won't open, a producer emits something odd, or a diff needs an object-level
explanation.

Deliberately *not* this tool: malicious-document triage (risk scoring, JavaScript extraction,
`/Launch` targets). That is already `blocks/pdf-object-analyzer`, which walks the same object
tree for a completely different output. Confirmed by reading
`blocks/pdf-object-analyzer/core/src/lib.rs` — its output type is `Analysis { indicators,
risk_level, javascript, launch_targets, … }`; it never reports the xref table, the trailer
dictionary, per-object dictionaries, or stream filter/length data. No other block in `blocks/`
mentions `xref` against a PDF (`csv-to-pdf-table`, `sbom-generator`, `sbom-diff` are the only
`xref` hits and are unrelated).

## Competitors reviewed

| # | Competitor | Shape | What it does well |
|---|-----------|-------|-------------------|
| 1 | qpdf `--json` (CLI, open source) | CLI → JSON | Full object graph as JSON: a header block with PDF version + highest object id, a `trailer` entry, and one entry per indirect object keyed by object/generation. Stream data inclusion is a tri-state option (omit / inline / external file). Object streams, xref streams, encryption and linearization dictionaries are all surfaced. Separate `--show-xref` / `--show-object` inspection modes. Filters: restrict output to one object, or to named top-level keys. |
| 2 | pdf-parser.py (CLI, forensics staple) | CLI → text | A statistics mode that counts comments, xref sections, trailers, `startxref` markers and indirect objects, plus a per-`/Type` census. Object selection by id. A free-text search across indirect objects. Raw vs. filtered (decoded) stream output as separate switches. |
| 3 | Browser-local PDF inspector pages (e.g. the pdfux-style "inspect PDF" page) | Web, client-side | Local, no-upload processing; drag-and-drop; an expandable tree of the internal structure; explicitly handles corrupt files by reporting the error instead of failing silently; no file-size cap advertised; works offline after first load. |
| 4 | Web "PDF source viewer" pages | Web | Raw source + object structure in one view; stream contents shown; cross-reference table and metadata called out as first-class sections. |
| 5 | Desktop structure browsers (iText RUPS, PDFXplorer, JPedal's inspector, PDF Gears class of tools) | Desktop GUI | Tree view of the COS object graph with a dedicated xref view; stream bodies viewable as text or hex; lookup of a specific object by id or by `/Type`; a separate view per structural concern (pages, form fields, outlines). |

## Table stakes → our decision

| Table stake (seen in ≥2 competitors) | Decision | Where it lands |
|---|---|---|
| PDF header version | **in-model** | `pdf_version` |
| Trailer dictionary shown as key→value | **in-model** | `trailer` (rendered PDF-syntax values, refs as `"12 0 R"`) |
| Cross-reference data: type (table vs. stream), declared `/Size`, `startxref` offset, per-entry offsets, free entries | **in-model** | `xref { kind, size, startxref, entry_count, in_use_entries, free_entries, compressed_entries, entries[] }` |
| Incremental-update count | **in-model** | `xref.incremental_updates` (counted from `startxref` markers in the raw bytes) |
| Per-`/Type` object census (pdf-parser `--stats`) | **in-model** | `object_types[]` |
| Indirect-object listing with id, kind, `/Type`, `/Subtype`, dictionary keys | **in-model** | `objects[]` |
| Dictionary rendered in PDF syntax | **in-model** | `objects[].dict` (truncated per entry, with an explicit `dict_truncated` flag) |
| Stream dictionaries: `/Filter` chain, declared `/Length`, stored byte length, decoded byte length | **in-model** | `streams[]` |
| Select a single object by id (`--json-object` / `-o`) | **in-model** | `object_id` param (accepts `12` or `12 0`) |
| Filter objects by a dictionary key or `/Type` (`-s`, `/Type` lookup) | **in-model** | `filter_key` param (matches `/Type` values and dictionary key names) |
| Choose which section to return (`--json-key`, per-view desktop tabs) | **in-model** | `section` enum: `all`, `summary`, `trailer`, `xref`, `objects`, `streams` |
| Cap on how much is listed | **in-model** | `max_objects` (default 100, 1–5000) — competitors page/scroll instead; a cap keeps a JSON answer readable and is stated on the page |
| Encryption + linearization flags | **in-model** | `encrypted`, `linearized` |
| Local, no-upload processing | **already our model** | browser wasm; the page never uploads |
| Corrupt-file behavior = a real error message | **in-model** | parse failure returns `failed to parse PDF: …`, stated on the page |
| Drag-and-drop / paste file input | **already shipped** | the shared file-input page control |
| Preset/one-click views | **in-model** | `[[example]]` chips, one per section view |

## Considered, out of model (listed, not built)

- **Stream body dumps (raw + filtered) and hex views.** qpdf can inline base64 stream data;
  pdf-parser and the desktop browsers show decoded stream text/hex. Dumping arbitrary stream
  bodies into a JSON answer is unbounded in size and is already served better by
  `pdf-extract-text` (text), `extract-pdf-images` (image XObjects) and
  `pdf-javascript-extractor` (script streams). We report each stream's *dictionary* plus raw and
  decoded byte counts, which is the structural fact; the bodies stay with the tools built for them.
- **Writing/repairing structure** (qpdf `--json-input` / `--update-from-json`, RUPS editing). This
  tool is read-only by design; `pdf-compress`, `pdf-organize` and `pdf-metadata-edit` cover
  rewrites.
- **Interactive expandable tree UI with lazy child loading.** The generator renders a single
  result region; a JSON report with a `section` selector and object-id lookup covers the same
  need declaratively without per-tool UI code.
- **Decrypting an encrypted document to inspect its objects.** We report `encrypted: true` and the
  `/Encrypt` dictionary reference; supplying a password is `protect-pdf`'s territory.
- **Content-stream operator disassembly** (JPedal's single-stepping). That is page rendering
  semantics, explicitly outside "without rendering the page content".

## Sources

- [qpdf JSON documentation](https://qpdf.readthedocs.io/en/stable/json.html)
- [pdf-parser.py — Didier Stevens](https://blog.didierstevens.com/programs/pdf-tools/)
- [pdfux inspect-pdf](https://pdfux.com/inspect-pdf/)
- [PDF source viewer (creshy)](https://creshy.com/tools/pdf-source-viewer/)
- [How to view PDF objects — IDR Solutions](https://blog.idrsolutions.com/how-to-view-pdf-objects/)
