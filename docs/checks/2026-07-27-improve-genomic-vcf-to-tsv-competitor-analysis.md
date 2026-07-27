# genomic-vcf-to-tsv competitor analysis (2026-07-27)

Goal: flatten Variant Call Format (VCF) text into a tidy TSV table, including INFO annotations and optional per-sample FORMAT/genotype fields.

## Scan

| Reference | Table-stakes observed | In model? | Decision |
| --- | --- | --- | --- |
| bcftools query / view workflows | Extract fixed VCF columns, select INFO tags, include FORMAT/sample values, and filter records such as PASS-only. | Yes | Implemented fixed columns, INFO whitelist, sample FORMAT extraction, and `pass_only` for `PASS`/`.` records. |
| Ensembl VEP / annotation table exports | Spreadsheet-friendly tabular output, explicit missing placeholders, and annotation columns suitable for downstream joins. | Partial | Generic INFO flattening and custom missing placeholders are in-model; interpreting VEP `CSQ` or `ANN` subfield schemas is deferred. |
| Online VCF-to-table converters | Paste/upload VCF, choose compact vs sample-expanded output, export TSV/CSV, and handle multi-sample records. | Yes | Pasted VCF text, long/wide sample layouts, header toggle, and browser/CLI TSV output are implemented. |

## Parameter decisions

- `input`: in-model. Paste text VCF keeps the tool pure, deterministic, and usable in chat/CLI/page surfaces.
- `layout`: in-model enum (`long`, `wide`). Long layout is better for sample-centric analysis; wide layout matches one-row-per-variant spreadsheets.
- `include_info`: in-model boolean. Competitors let users extract annotations; default true.
- `include_samples`: in-model boolean. Multi-sample genotype columns are table stakes; default true.
- `info_fields`: in-model string. Allows selected INFO keys such as `DP,AF,AC` without implementing a full query language.
- `pass_only`: in-model boolean. Common quality-control filter for `FILTER=PASS` or unfiltered `.` calls.
- `prefix_info`: in-model boolean. Disambiguates collisions like INFO `DP` and FORMAT `DP`.
- `missing`: in-model string. Lets users choose `.`/`NA`/blank-compatible placeholders.
- `header`: in-model boolean. Supports append-only pipelines that already have a header row.

## Out-of-model / deferred

- `.vcf.gz`/BCF decoding and tabix indexing: requires compression/index plumbing and file upload semantics beyond this pure text tool.
- Variant normalization, left alignment, reference FASTA validation, transcript-aware annotation parsing, and VEP/ANN subfield expansion: domain-specific genomic model/tooling scope, not a generic TSV flattener.
- Huge cohort VCF streaming: browser paste fields are for short to moderate diagnostic extracts; large cohort processing belongs in command-line bioinformatics pipelines.

## Verification plan

- Core unit tests cover long and wide layouts, INFO whitelist/prefixing, flags, custom missing placeholders, PASS-only filtering, inferred sample names, header suppression, malformed input errors, and bad layout errors.
- CLI matrix asserts exact output for long layout and exercises wide layout, selected INFO fields, PASS-only, custom missing, header=false, and disabled sample/info controls.
- Page test asserts real TSV output and deep-link prefill behavior.
