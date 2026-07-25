# pdf-metadata-edit — Competitor Analysis (2026-07-25)

A survey of five widely used tools/services for reading and editing PDF document
metadata (Title, Author, Subject, Keywords, and related fields), spanning desktop
apps, online services, and command-line utilities. All findings below are
paraphrased from public product/documentation pages (sources listed at the end);
no marketing copy or branding is reproduced.

## Competitors

| Tool | Fields editable | View vs. edit | Free / Online / CLI | Notable features |
|------|-----------------|---------------|---------------------|------------------|
| Adobe Acrobat (Document Properties) | Title, Author, Subject, Keywords editable; Creator, Producer, PDF version, file size shown read-only; full XMP via an "additional metadata" panel | Both — free Reader views only; editing needs the paid tier | Paid desktop (viewing free) | Reached via Document Properties → Description; keywords entered comma-separated; deep, standards-compliant XMP support |
| Sejda – Edit PDF Metadata | Title, Author, Subject, Keywords | Both — shows existing values on upload, then lets you change them | Free online (page/size/task caps) | No install or signup; simple upload → edit → download; no watermark |
| iLovePDF | Title, Author, Subject, Keywords, Creator, Producer, plus dates | Both — shows current values, supports editing and removing fields | Free tier online; desktop apps | Can wipe individual fields or clear metadata entirely; privacy-oriented; part of a broader toolkit |
| exiftool (CLI) | Title, Author, Subject, Keywords, Creator, Producer, CreateDate, ModifyDate, Trapped | Both — reads all tags and writes them | Free, open-source CLI | Fast batch edits over a folder without re-rendering; can strip all metadata; in-place overwrite; scriptable |
| BeCyPDFMetaEdit | Author, Title, Subject, Keywords; also viewing prefs, bookmarks, page labels | Both — a metadata tab auto-shows title/author/subject/creation date/app | Free desktop app with a CLI batch mode | Three write modes (incremental, full overwrite, repair); batch file lists; can also remove passwords |

(pdfcpu was also reviewed — its CLI reads Info-dict fields reliably and can set
some properties, but complete/consistent editing of every Info-dict field has
historically been a limited area.)

## Common capabilities

The baseline shared across nearly all tools:

- Editing the four standard Info-dictionary fields: Title, Author, Subject,
  Keywords.
- Viewing existing metadata before editing (current values shown on load).
- Treating Creator and Producer as read-only / auto-populated by the generating
  application.
- Keywords entered as a single comma-separated string.
- A simple load → edit → save/download workflow.

## Gaps / opportunities

Differentiators of stronger tools, and room for a minimal tool to grow into:

- Batch processing — editing many PDFs at once (exiftool, BeCyPDFMetaEdit) vs.
  the one-at-a-time online tools.
- Clear-vs-set semantics — explicitly wiping a field vs. leaving it untouched
  vs. overwriting (iLovePDF and exiftool handle removal cleanly).
- Full XMP / custom metadata — only Acrobat and exiftool reach beyond the four
  basic fields.
- Date handling — editing CreateDate / ModifyDate.
- Editing Producer / Creator — most tools keep these read-only.
- In-place / no-reupload editing and privacy — CLI tools keep files local.
- Friendly keyword UX — normalizing a comma-separated list rather than raw entry.

## Recommendation for a minimal viable tool

A first version should cover the shared baseline every competitor offers, with
clear, predictable semantics:

- View mode — read and display the existing Info-dictionary fields (Title,
  Author, Subject, Keywords) plus read-only context fields (Creator, Producer)
  where present.
- Edit mode — set or clear Title, Author, Subject, Keywords, with an intuitive
  rule: an empty/omitted input leaves the existing value untouched (so a user can
  update one field without wiping the others). The core additionally supports an
  explicit clear that removes a field.
- Keyword handling — accept a comma-separated string, matching the convention
  every other tool uses.

This matches the entry-level capability of Sejda and Acrobat's Description tab
while staying minimal. Natural follow-ons once the baseline is solid: batch
editing, editable dates, Producer/Creator overrides, and XMP/custom properties.

## Sources

- Sejda – Edit PDF metadata: https://www.sejda.com/edit-pdf-metadata
- iLovePDF – Edit or remove PDF metadata: https://www.ilovepdf.com/blog/edit-or-remove-pdf-metadata-windows-mac
- exiftool PDF tag names: https://web.mit.edu/graphics/src/Image-ExifTool-6.99/html/TagNames/PDF.html
- Adobe Acrobat – Document properties and metadata: https://helpx.adobe.com/acrobat/desktop/edit-documents/edit-pdf-properties/pdf-properties.html
- BeCyPDFMetaEdit – MobileRead Wiki: https://wiki.mobileread.com/wiki/BeCyPDFmetaedit
- pdfcpu – Add properties: https://pdfcpu.io/properties/properties_add.html
