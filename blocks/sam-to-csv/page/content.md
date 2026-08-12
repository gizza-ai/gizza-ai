## About this tool

SAM (Sequence Alignment/Map) files are tab-separated alignment records. They are easy for aligners and genome viewers to read, but awkward to paste into a spreadsheet or inspect in a notebook because the columns are positional and the bitwise `FLAG` field hides useful facts such as paired, unmapped, reverse-strand, secondary or supplementary status.

Paste SAM text here and the tool turns each alignment line into CSV, TSV, semicolon-separated or pipe-separated rows with named columns. Header lines such as `@HD`, `@SQ`, `@RG`, `@PG` and `@CO` are skipped. Optional `TAG:TYPE:VALUE` fields can be expanded into their own columns, joined into one `TAGS` cell, or dropped.

### Worked example

Input:

```text
@HD	VN:1.6	SO:coordinate
r001	99	chr1	7	60	8M2I4M1D3M	=	37	39	TTAGATAAAGGATACTG	*	NM:i:1	AS:i:30
```

Default output starts with:

```csv
QNAME,FLAG,RNAME,POS,MAPQ,CIGAR,RNEXT,PNEXT,TLEN,SEQ,QUAL,FLAG_SUMMARY,NM,AS
r001,99,chr1,7,60,8M2I4M1D3M,=,37,39,TTAGATAAAGGATACTG,*,"PAIRED,PROPER_PAIR,MATE_REVERSE,READ1",1,30
```

Turn on **Add END, REF_SPAN, READ_LEN, STRAND** and the same record gets `END=22`, `REF_SPAN=16`, `READ_LEN=17` and `STRAND=+` from `POS`, `CIGAR`, `SEQ` and the reverse-strand flag.

### FLAG decoding modes

- **Summary column**: one `FLAG_SUMMARY` cell listing the set bit names.
- **12 true/false bit columns**: one column per SAM bit (`FLAG_PAIRED`, `FLAG_UNMAPPED`, `FLAG_REVERSE`, and so on).
- **Summary + bit columns**: useful when a table is for both humans and filters.
- **Raw FLAG only**: smallest output.

### Limits and edge cases

- Maximum input is **20,000 alignment records** per run.
- This parses text SAM only. It does not read BAM/CRAM binaries, index files, FASTA references or remote URLs.
- Each alignment record must contain the 11 mandatory SAM fields. Header-only input is rejected with a clear message.
- Optional tags must be in `TAG:TYPE:VALUE` form, for example `NM:i:0` or `MD:Z:36`.
- Space-separated paste is tolerated when at least the first 11 fields survive, but real SAM tabs are safest.
- CIGAR validation is limited to syntax and span calculations; the tool does not check the record against a reference genome.

## FAQ

<details>
<summary>What SAM columns are included?</summary>

The base table uses the 11 mandatory SAM alignment fields: `QNAME`, `FLAG`, `RNAME`, `POS`, `MAPQ`, `CIGAR`, `RNEXT`, `PNEXT`, `TLEN`, `SEQ` and `QUAL`. You can drop `SEQ` and `QUAL` for a compact coordinate table, add decoded flag columns, add computed span columns, and keep optional tags as either one `TAGS` column or one column per tag.

</details>

<details>
<summary>How is the FLAG field decoded?</summary>

`FLAG` is a bitwise sum. The tool checks the standard bits: paired, proper pair, unmapped, mate unmapped, reverse strand, mate reverse, read1, read2, secondary, QC fail, duplicate and supplementary. In summary mode, the set names are joined in one cell; in bit mode, each bit gets a true/false column.

</details>

<details>
<summary>Can this convert BAM or CRAM?</summary>

No. BAM and CRAM are binary formats and CRAM may also need a reference genome. This tool is intentionally a lightweight browser-safe SAM text parser. Convert BAM/CRAM to SAM with a bioinformatics tool first, then paste the SAM records here.

</details>

<details>
<summary>What do END and REF_SPAN mean?</summary>

When **Computed** is on, `REF_SPAN` is the number of reference bases consumed by the CIGAR operations `M`, `D`, `N`, `=` and `X`. `END` is `POS + REF_SPAN - 1`, the last reference coordinate covered by the alignment. Insertions and soft clips affect the read length but not the reference span.

</details>

<details>
<summary>How do I keep only the tags I care about?</summary>

Set **Optional SAM tags** to **Expand one column per tag** or **Join into TAGS column**, then put a comma-separated list in **Tag whitelist/order**, for example `NM,AS,MD`. That both filters the tags and fixes their output order, which is helpful when combining multiple runs.

</details>
