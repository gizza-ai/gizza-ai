# gmail-takeout-parser — competitor analysis (2026-07-10)

Function: parse a Google Takeout Gmail **mbox** export into a clean **CSV / JSON**
table of messages, one row per message, with the Gmail **labels** preserved.

## Competitors scanned (top real tools)

1. **mboxtocsv.com — "MBOX to CSV Converter for Mac"** — desktop app. Exports
   From, To, Cc, Bcc, Subject, Date, Body to CSV / Excel / JSON. Dates
   standardized to a consistent format. Free under 50 MB. (mboxtocsv.com)
2. **GMail-MBox-Mimeparse (petebromberg, GitHub)** — console app that turns a
   Google Takeout Gmail mbox into CSV, specifically surfacing the Gmail
   `X-Gmail-Labels` header so labels/folders are kept as a column. (github.com)
3. **GroupDocs Online MBOX→CSV / BitRecover / Aid4Mail** — bulk converters:
   upload mbox, pick CSV, download a per-message table with Date, From/To,
   Subject columns; advanced date/sender/subject filters in the paid tiers.
   (products.groupdocs.app, bitrecover.com)

## Table-stakes (each mapped to an in-model / out-of-model decision)

| Capability | Competitors | Decision |
|---|---|---|
| One row per message | all | **in** — split the mbox on `From ` postmark lines |
| From / To / Cc columns (name + address) | all | **in** — `mail-parser`, formatted `Name <addr>` |
| Subject column | all | **in** |
| Date column, standardized | all | **in** — normalized to ISO-8601 (RFC 3339) |
| Gmail **labels** column (`X-Gmail-Labels`) | GMail-MBox-Mimeparse | **in** — the differentiator; parsed from the header |
| Message-ID column | forensic tools | **in** |
| CSV output | all | **in** — RFC-4180 quoting |
| JSON output | mboxtocsv.com | **in** — `format=json` |
| Body / snippet column | mboxtocsv.com | **in** — optional (`include_body`), length-capped (`snippet_chars`) |
| Bulk / multi-file, whole Takeout archive | BitRecover, Aid4Mail | **out** — this tool takes one pasted mbox; huge multi-GB archives exceed browser memory (documented as a limit) |
| Attachment **file** extraction | BitRecover, turgs | **out** — table tool; `eml-parse` inspects a single message's attachments |
| Excel `.xlsx` export | mboxtocsv.com | **out** — CSV covers spreadsheets; the separate `csv-to-xlsx` tool wraps it |
| PST / PDF / EML export | BitRecover, BLR | **out** — different output class |
| Date / sender / subject **filters** | Aid4Mail, GroupDocs paid | **out (v1)** — export the full table; users filter in their spreadsheet. Noted as a future add. |

## UX patterns adopted

- **Format select** (`csv` / `json`) with friendly `[input.labels]`.
- **Include body** checkbox (off by default — keeps the table compact).
- **Snippet length** number field (chars; `0` = full body) with a slider.
- Worked example + `[[example]]` preset chip prefilling a two-message mbox.

No competitor copy, branding, or trademarks were reused — behavior only.

Sources:
- <https://mboxtocsv.com/>
- <https://github.com/petebromberg/GMail-MBox-Mimeparse>
- <https://products.groupdocs.app/conversion/mbox-to-csv>
- <https://www.bitrecover.com/mbox-to-csv/>
- <https://forensiksoft.com/blog/convert-gmail-takeout-labels/>
