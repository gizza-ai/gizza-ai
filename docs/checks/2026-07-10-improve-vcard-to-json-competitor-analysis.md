# vcard-to-json — competitor analysis (2026-07-10)

Scan performed before implementing. One WebSearch ("vcard to json converter online
vcf parser tool"); skimmed the top real competitor tools. All copy below is
**paraphrased** — no competitor copy/branding/trademarks reproduced.

## Competitors skimmed

1. **vcfconverter.com — VCF to JSON** — client-side (browser) VCF↔CSV/Excel/JSON.
   Emphasises "files never leave your device". Paste or upload; JSON output.
2. **IO Tools — VCF (vCard) to JSON** — parses vCard 3.0 (RFC 2426) and 4.0
   (RFC 6350) fully in-browser. Handles line folding, multi-contact files,
   structured Name and Address values, multi-TYPE params. Output options:
   pretty-print, flat array vs. wrapped object, structured Name/Address splitting,
   ISO date parsing.
3. **ConversionTab — vCard to JSON** — paste or upload; "designed for modern APIs".
4. **converter.app / aconvert — VCF to JSON** — no-registration file conversion.
5. **npm `vcf-to-json-converter`** — programmatic; flat JSON, jCard (RFC 7095),
   field mapping, multi-contact handling.

## Table-stakes params / behaviours (tagged for gizza's browser-local wasm model)

| capability | in-model? | decision |
| --- | --- | --- |
| Parse vCard 3.0 + 4.0 | in-model | built — parser handles both |
| Line unfolding (folded continuation lines) | in-model | built |
| Multi-contact files (repeated BEGIN/END) | in-model | built → JSON array |
| Structured N split (prefix/given/middle/family/suffix) | in-model | built, `structured` toggle |
| Structured ADR split (poBox/ext/street/locality/region/postalCode/country) | in-model | built, `structured` toggle |
| Multi-TYPE params (`TYPE=work,voice`, + 2.1 bare-type form) | in-model | built → `types` array |
| Repeatable props → arrays (emails/phones/urls/addresses) | in-model | built |
| Pretty-print JSON | in-model | built, `pretty` param |
| Flat array vs. wrapped object output | in-model | built, `wrap` enum (array/object) |
| Value unescaping (`\n \, \; \\`) | in-model | built |
| Group prefixes (`item1.TEL`) stripped | in-model | built |
| Paste input | in-model | built (page textarea) |

## Considered, not built

- **jCard (RFC 7095) output** — a distinct serialization standard (array-of-arrays
  per property with type tags). In-model but a separate spec with meaningful
  complexity; deferred to keep this tool's JSON model clean and focused. Recorded
  as a future capability rather than forced in.
- **File upload** — the page accepts pasted `.vcf` text (multiline); a drag-drop
  file picker is a page-UX nicety, not a parsing capability. Out of scope here.
- **CSV/Excel output** — a different converter (gizza already ships
  `csv-to-vcard` for the reverse tabular direction); not this tool's job.

## Design decisions

Descriptor params: `data` (required text), `pretty` (bool, default false),
`structured` (bool, default true), `wrap` (enum array|object, default array).
Every fixed-choice param is an enum; every param carries a `.describe()`.
