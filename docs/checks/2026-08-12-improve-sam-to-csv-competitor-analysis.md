# sam-to-csv competitor analysis (2026-08-12)

Backlog row: `sam-to-csv` — parses SAM sequence-alignment records into a CSV table with named columns plus optional decoded flag bits.

## Competitor scan

Search query: "SAM to CSV converter online SAM flag decode CIGAR optional tags".

| Competitor / reference | Observed table-stakes | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| SAMtools / htslib command-line workflows | Read SAM/BAM/CRAM, filter by FLAG/MAPQ, expose columns, compute alignment fields. | Text SAM parsing and filters fit. BAM/CRAM, indexes and reference genomes are out of model for a lightweight browser block. | Parse SAM text only; add mapped/primary/MAPQ filters and computed span fields. |
| Bioinformatics flag-decoder pages | Decode numeric SAM FLAG into paired/unmapped/reverse/secondary/supplementary bit meanings. | Yes. The 12 standard bits are deterministic. | Add `summary`, `bits`, `both`, and `none` FLAG modes. |
| Spreadsheet/manual awk recipes | Convert tab-separated mandatory SAM columns to CSV/TSV with headers. | Yes. | Always name the 11 mandatory columns and offer comma, tab, semicolon and pipe delimiters with CSV quoting. |
| SAM/BAM viewer tools | Show optional tags such as NM/AS/MD and allow filtering/searching records. | Partly. Optional tag parsing fits; indexed browsing and binary formats do not. | Support `tags=expand|joined|none` plus `tag_fields` whitelist/order. |
| CIGAR calculators | Compute reference span/end coordinates and read length from CIGAR. | Yes for syntax-level calculations. Reference validation is out of model. | Add `computed=true` for END, REF_SPAN, READ_LEN and STRAND. |

## In-model feature set shipped

- Input: pasted SAM text; `@` header lines skipped.
- Mandatory SAM fields: QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL.
- Delimiters: comma, tab, semicolon, pipe.
- Optional header row.
- FLAG decode: summary, bit columns, both, or none.
- Optional tags: expanded one column per tag, joined into TAGS, dropped, and optional whitelist/order.
- Filters: mapped only, primary only, minimum MAPQ.
- Computed columns: END, REF_SPAN, READ_LEN, STRAND.
- 20,000-record cap.

## Out-of-model or deferred

- BAM/CRAM binary parsing and indexes.
- Fetching remote files or streaming large alignment files.
- Reference-genome validation.
- Sorting, indexing, pileups, variant calling, or interval operations.

## Verification snapshot

- Core tests cover mandatory columns, FLAG summary/bits, optional tags, computed CIGAR spans, filters, CSV quoting, malformed records, bad CIGARs and invalid options.
