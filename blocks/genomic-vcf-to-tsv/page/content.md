## About this tool

VCF files are compact for variant callers, but awkward in spreadsheets: the fixed columns are followed by semicolon-delimited INFO annotations and, often, colon-delimited FORMAT values for each sample. This tool turns those records into tidy TSV so you can inspect, filter, or join variants with ordinary table tools.

The output always starts with `CHROM`, `POS`, `ID`, `REF`, `ALT`, `QUAL`, and `FILTER`. When enabled, INFO fields are expanded into one column per key. Sample FORMAT values can be written in **long** layout (one row per variant × sample with a `SAMPLE` column) or **wide** layout (one row per variant with columns such as `NA001_GT` and `NA001_DP`).

### Worked example

Paste this VCF:

```text
##fileformat=VCFv4.2
#CHROM	POS	ID	REF	ALT	QUAL	FILTER	INFO	FORMAT	NA001	NA002
chr1	100	rs1	A	G	50	PASS	DP=30;AF=0.5	GT:DP	0/1:20	1/1:10
```

With the default long layout, the TSV contains two data rows, one for `NA001` and one for `NA002`, with INFO columns `DP` and `AF` plus FORMAT columns `GT` and `DP`.

### Controls

- **Sample layout** chooses long rows for sample-centric analysis or wide rows for variant-centric spreadsheets.
- **Explode INFO fields** adds one column per INFO key; use **INFO keys to keep** such as `DP,AF,AC` when you only need selected annotations.
- **Include sample FORMAT fields** emits genotype/sample values when the VCF has sample columns.
- **PASS-only** drops filtered calls and keeps records whose `FILTER` is `PASS` or `.`.
- **Prefix INFO columns** writes `INFO_DP` instead of `DP`, useful when INFO and FORMAT have the same key.
- **Missing value** controls the placeholder for absent INFO/FORMAT/sample values.

### Limits and edge cases

This is a table flattener, not a genomic normalizer. It does not split multi-allelic ALT values, left-align variants, query reference FASTA files, parse compressed `.vcf.gz`, or validate VEP/ANN subfield semantics. Convert compressed files to text first and paste a representative VCF section. Values are made TSV-safe by replacing embedded tabs/newlines with spaces.

## FAQ

<details>
<summary>Does this support multi-sample VCF files?</summary>

Yes. If the `#CHROM` header has sample names after `FORMAT`, long layout emits one row per sample per variant, while wide layout creates `<sample>_<FORMATKEY>` columns for each discovered FORMAT key.

</details>

<details>
<summary>What happens to INFO flags such as DB?</summary>

A bare INFO flag with no equals sign is emitted as `true` when present. If it is absent on another record, the configured missing-value placeholder is used.

</details>

<details>
<summary>Can I keep only a few INFO fields?</summary>

Yes. Put a comma-separated list such as `DP,AF,AC` in **INFO keys to keep**. The output columns follow that order and missing keys get the configured placeholder.

</details>

<details>
<summary>Does this replace bcftools or VCF normalization?</summary>

No. Use specialized bioinformatics tools for normalization, reference lookups, compressed VCF indexing, and annotation interpretation. This tool focuses on deterministic text flattening into TSV inside the browser.

</details>
