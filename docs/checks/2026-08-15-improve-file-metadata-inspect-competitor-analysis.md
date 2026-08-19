# file-metadata-inspect — competitor analysis (2026-08-15)

Scan run before completing the partial build, per `create-next-tool`. One web
search ("online file metadata viewer EXIF PDF document properties metadata
inspector") surfaced cross-format and PDF-specific metadata viewers. Everything
below is a paraphrased feature inventory — no competitor copy, wording, branding
or trademarks is reused in the tool.

## Competitors skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | metadataview.com — multi-format metadata viewer | yes | Upload-based viewer for images, PDFs, videos, audio and documents; advertises EXIF, GPS, XMP, colour/profile data, PDF/Office author fields and timestamps. |
| 2 | metadata2go.com — file metadata checker | yes | Drag-and-drop upload across images, documents, videos, audio and ebooks; returns hidden metadata fields and a general file-type summary. |
| 3 | pics.io PDF metadata viewer | yes | PDF-focused upload tool; shows document properties such as version, page count, creator/producer, author/title and creation/modification dates. |

All three are upload-to-service tools. The gizza tool keeps the same inspection
job but runs as a deterministic block over the provided bytes and returns a
machine-readable report.

## Table-stakes inventory

| Capability | Seen on | In model? | Where it landed |
|---|---|---|---|
| Detect the container from bytes, not just extension | 1, 2 | in | `core::detect_container` sniffs JPEG/PNG/TIFF/WebP/HEIF/AVIF, PDF and ZIP-family containers. |
| Show image EXIF/TIFF tags | 1, 2 | in | `kamadak-exif` group named `EXIF`, including camera, software, exposure and timestamps. |
| Decode image GPS coordinates | 1, 2 | in | EXIF GPS latitude/longitude are converted to decimal degrees and a privacy note is emitted. |
| Extract XMP packets | 1, 2 | in | Raw-byte XMP scan pulls common `dc:*`, `xmp:*`, `photoshop:*`, `pdf:*` fields, including RDF list values. |
| Show PDF document information | 1, 2, 3 | in | `lopdf` reads the trailer `/Info` dictionary, PDF version, page count and encryption state. |
| Show Office/OpenDocument properties | 1, 2 | in | ZIP container reader parses OOXML `docProps/core.xml`/`app.xml` and ODF `meta.xml`. |
| E-book metadata | 2 | in | EPUB OPF metadata is discovered via `META-INF/container.xml` and parsed from the package document. |
| Graceful unsupported-file result | 2 | in | Unknown or metadata-free files return a "no supported metadata found" summary rather than failing. |
| Huge-format coverage via ExifTool-style external binaries | 1, 2 | out | The block must be pure Rust/wasm and cannot shell out to ExifTool. Unsupported formats are named as limits. |
| Video/audio codec metadata | 1, 2 | out | Existing media-info/video metadata blocks cover ffprobe-style stream metadata; this pure block stays document/image focused. |
| Metadata removal/editing | adjacent tools | out | Existing `strip-exif`, metadata edit and PDF edit rows cover mutation; this tool is read-only inspection. |

Nothing from the scan was dropped silently: every table-stake is either in the
descriptor/report shape or listed as out-of-model / delegated to an existing
sibling tool family.

## Design decisions taken from the scan

- **One report with grouped fields.** Competitors present format-specific
  sections; the block mirrors that with `groups` such as `EXIF`, `XMP`, `PDF
  Info`, `Document properties`, and `EPUB metadata`.
- **Byte sniffing first.** Metadata viewers work when a file is misnamed; the
  implementation routes by magic bytes and ZIP members rather than trusting the
  URL suffix.
- **Privacy without overreach.** GPS gets a dedicated decoded field and note,
  but values are not redacted because this is an inspection tool; privacy-risk
  redaction/reporting remains `metadata-privacy-linter`.
- **Partial failure is useful.** If a PDF trailer or ZIP member is malformed, the
  report records a note and still returns any metadata block it could read.
- **Bounded output.** Long values and huge field lists are capped so a single
  camera raw or verbose XMP packet cannot swamp chat or CLI output.

## Not copied

No competitor copy, option labels, branding or FAQ wording appears in the block.
The implementation follows public file-format structures (EXIF/TIFF, PDF Info,
XMP, OOXML/ODF/EPUB XML) and emits neutral JSON field names.
