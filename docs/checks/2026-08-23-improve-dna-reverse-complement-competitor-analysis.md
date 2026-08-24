# dna-reverse-complement — competitor analysis (2026-08-23)

Scan run **before** implementation, per `/create-next-tool` step 4. Findings are paraphrased
observations of publicly documented behaviour only — no competitor copy, branding, or trademarks
were reproduced, and nothing was copied into the page.

## Tools skimmed

| # | Tool | Reachable | What it does |
|---|------|-----------|--------------|
| 1 | reverse-complement.com | yes | Single textarea, browser-local reverse complement of DNA/RNA, FASTA-aware |
| 2 | Sequence Manipulation Suite `rev_comp` (bioinformatics.org/sms2) | yes | Raw or multi-FASTA input, three separate operations, full IUPAC alphabet |
| 3 | Harvard `revcomp` utility (arep.med.harvard.edu) | yes | Multi-sequence (one per line), documents the IUPAC degeneracy table it follows |

Search also surfaced several newer SaaS-style calculators (SciDataUtils, RunCell, LabTools,
molecularlabtools) whose feature lists match the three above: DNA+RNA, IUPAC codes, reverse-only /
complement-only modes, case preservation. They were not needed as substitutes since all three
primary targets were reachable.

## Table stakes observed

| # | Capability | Seen on | Decision |
|---|-----------|---------|----------|
| 1 | Reverse complement of a pasted sequence | 1, 2, 3 | **in-model** — the default `operation` |
| 2 | Separate *complement-only* and *reverse-only* operations | 2, 3 | **in-model** — `operation` enum has all three |
| 3 | Full IUPAC ambiguity alphabet (R/Y/S/W/K/M/B/D/H/V/N) | 1, 2, 3 | **in-model** — complement table covers every code |
| 4 | Correct degenerate pairing: R↔Y, K↔M, B↔V, D↔H, S→S, W→W, N→N | 1, 3 | **in-model** — 3 explicitly notes S/W are self-complementary, a known error source; unit-tested |
| 5 | RNA input (U) accepted; DNA/RNA output choice | 1, 2 | **in-model** — `output_alphabet` = auto/dna/rna (1 always emits DNA; auto keeps the input's alphabet, which is the friendlier default) |
| 6 | Case preserved so lower-case can mark regions of interest | 1, 2 | **in-model** — `preserve_case` (default on) |
| 7 | FASTA input, header preserved, multi-record handled per record | 1, 2 | **in-model** — auto-detected from a leading `>`; each record complemented independently |
| 8 | Whitespace / line breaks in pasted sequence tolerated | 1, 3 | **in-model** — inter-base whitespace is always stripped, then re-wrapped |
| 9 | Everything runs client-side, no upload | 1 | **in-model** — every gizza page is browser-local wasm by construction |
| 10 | Output line wrapping for FASTA readability (60 is the convention) | 2 (implicit in its FASTA output) | **in-model** — `line_width`, default 0 = one line, 60 = FASTA convention |
| 11 | Reference table of the IUPAC codes shown next to the tool | 3 | **in-model as page copy** — the code table lives in `page/content.md` |
| 12 | Large input tolerance (2 documents a 100 M character cap) | 2 | **in-model, smaller cap** — 1,000,000 sequence characters, stated on the page; a browser tab is not a cluster node |

## Gaps we close that competitors leave open

- **Unknown-character policy is explicit.** All three competitors silently pass through or ignore
  junk characters (digits from numbered GenBank-style listings, `*`, stray punctuation). We expose
  `on_invalid` = `error` (default — say what was wrong and where), `drop`, or `keep`, so a
  numbered/annotated paste is a one-setting fix instead of a silently wrong answer.
- **Composition stats.** `show_stats` appends length, GC%, ambiguous-code and gap counts, which
  otherwise means a second tool.
- **Deep-linkable + CLI.** Every parameter is a query param on the page and a flag on the `gizza`
  CLI; competitors are page-only.

## Out-of-model (listed, deliberately not built)

| Capability | Why out of model |
|-----------|------------------|
| Six-frame translation / ORF finding on the reverse strand | A different tool's job (protein translation), not a reverse-complement block; would double the schema |
| Restriction-site or primer analysis on the revcomp product | Needs an enzyme/primer database; belongs in its own block |
| Alignment of the input against the reverse complement | Requires an aligner; out of scope for a pure string transform |
| File upload of multi-MB FASTA / FASTQ from disk | The page's field surface takes pasted text; FASTQ already has `blocks/fastq-to-fasta` upstream of this tool |
| Sequence feature annotation tracks (GenBank features) | Needs a GenBank parser + feature model, far beyond a complement transform |

## Resulting descriptor

`sequence` (required), `operation` (reverse_complement | complement | reverse),
`output_alphabet` (auto | dna | rna), `preserve_case` (bool, default true),
`line_width` (0–200, default 0), `on_invalid` (error | drop | keep), `show_stats` (bool,
default false). Every table stake above is either in this descriptor or in the out-of-model table —
none were dropped silently.
