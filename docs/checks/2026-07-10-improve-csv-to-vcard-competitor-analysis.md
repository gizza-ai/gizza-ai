# csv-to-vcard competitor analysis (2026-07-10)

Tool: `csv-to-vcard` — convert a CSV/JSON contact list into a single `.vcf` vCard file for bulk import.

## Competitors scanned

| Competitor | Table-stakes observed | In model? | Decision |
| --- | --- | --- | --- |
| FileConverts CSV to vCard | Upload CSV, choose delimiter/input options, emits `.vcf` for address books/Outlook-style imports. | Yes | Added `delimiter=auto|comma|semicolon|tab|pipe`, CSV header parsing, `.vcf` text output. |
| CoolUtils CSV to VCF | File upload flow, output-format choice, conversion options for contact spreadsheet → VCF. | Mostly | Built conversion locally from pasted data; file upload/download chrome is platform-level/out-of-model for the block, but text output can be saved as `.vcf`. |
| vcfconverter.com CSV to VCF | Workflow centered on Excel CSV exports and conversion to vCard for import. | Yes | Header auto-mapping supports common Excel/contact-export names (`First Name`, `Last Name`, `Email`, `Mobile Phone`, `Company`). |
| ConversionTab CSV/VCF | Paste or sample data, adjust input options, map contact fields to vCard properties. | Partly | Implemented automatic mapping for common columns rather than a full manual field-mapping UI; documented unknown columns are ignored. |

## In-model capabilities implemented

- CSV with a header row and auto delimiter sniffing.
- JSON array or single JSON object input for contact exports from APIs/CRMs.
- Common contact column mapping: name parts, full name, email, phone/mobile/fax, company, department, title, address, website, birthday, notes, gender.
- vCard `3.0` (default) and `4.0` output.
- RFC-style value escaping and 75-octet line folding.
- Multi-contact output in one `.vcf` text stream.
- Page controls for input format, delimiter, and vCard version; example chips for CSV and JSON.
- CLI and page examples that can be copied and run.

## Out-of-model / intentionally not built

- Direct file upload/download naming as a binary `.vcf` attachment: current gizza pure text page displays output for copy/save; browser download helpers may be added platform-wide.
- Manual drag-and-drop field mapping UI: feasible in a bespoke app, but outside the current descriptor model. Automatic mapping covers common exports.
- Contact photo embedding and binary attachments: not a good fit for text-only pure conversion and would require file/binary input.
- Online address book integration (Google/Microsoft import APIs): network/account auth is outside the offline gizza model.

## Verification plan

- Core exact-output test for a basic CSV contact.
- Error tests for empty input, unrecognized headers, bad JSON, and bad version.
- CLI exact-output run for the same basic CSV.
- Page Playwright test for exact vCard output plus deep-link query params.
