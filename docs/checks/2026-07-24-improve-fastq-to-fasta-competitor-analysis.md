# fastq-to-fasta — competitor analysis (2026-07-24)

Tool: strip per-base quality lines from FASTQ reads and emit clean FASTA, with
optional length + quality filtering. Pure text transform, fully in-model (Rust,
browser-local, no server). Sources reviewed (paraphrased only — no copy/branding
reused):

- FASTX-Toolkit `fastq_to_fasta` (hannonlab.cshl.edu/fastx_toolkit) — the canonical CLI.
- sequenceconversion.bugaco.com — BioPython-backed online converter.
- punnettsquare.org/fastq-to-fasta — free online converter, batch, keeps headers.
- proteiniq.io/app/fastq-to-fasta — free online, quality filtering + batch + header preservation.
- metagenomics.wiki / biostars threads — canonical one-liners + user expectations.

## FASTQ → FASTA background

FASTQ = 4 lines/read: `@id [desc]` / sequence / `+[id]` / per-base quality. FASTA
= `>id [desc]` / sequence (optionally wrapped). Conversion drops the `+` and
quality lines and rewrites `@` → `>`. Quality chars encode Phred scores as
`ord(char) - offset`; offset 33 = Sanger / Illumina 1.8+ (modern default), offset
64 = old Illumina 1.3–1.5.

## Table-stakes params (each → descriptor OR out-of-model)

| Feature | Competitor(s) | Decision |
| --- | --- | --- |
| Strip quality, `@`→`>`, keep header + description | all | **core** (always on) |
| Rename headers to sequential numbers (`-r`) | FASTX | in-model → `rename` bool |
| Discard reads containing ambiguous `N` (FASTX discards by default; `-n` keeps) | FASTX | in-model → `strip_n` bool (default false = keep N, friendlier for a general converter; documented) |
| Quality offset 33/64 (`-Q33`) | FASTX | in-model → `quality_offset` enumv (33/64) |
| Minimum read length filter | proteiniq, common one-liners | in-model → `min_length` int |
| Minimum mean-quality filter | proteiniq, punnettsquare | in-model → `min_quality` number (mean Phred) |
| Line wrapping at N chars (60/70/80 typical for FASTA) | seqkit/biopython convention | in-model → `wrap` int (0 = one line) |
| Uppercase sequence | seqkit convention | in-model → `uppercase` bool |
| Preserve original headers/descriptions | all | **core** (default; `rename` opts out) |
| Batch / per-line paste of many reads | all online tools | **core** — the whole multi-read input is one paste |

### Out-of-model (considered, not built)

- **Gzip (`.gz`) output** (`-z`) — binary output doesn't fit the text page render
  model; users can gzip the copied FASTA themselves.
- **Multi-file batch upload** — the page/CLI take one input; paste concatenated
  reads instead (all reads in one FASTQ blob convert together).
- **Verbose read-count report to a side channel** (`-v`) — folded into the always-on
  behavior; counts aren't a separate output stream here.

## UX decisions

- `quality_offset` renders a `<select>` (33 = Sanger/Illumina 1.8+, 64 = old Illumina).
- `min_length`, `min_quality`, `wrap` are number fields with real placeholders and
  bounded ranges (wrap ≤ 1000, the exact-cap boundary tested).
- `rename`, `strip_n`, `uppercase` are checkboxes (default off).
- `[[example]]` preset chips: basic convert, wrapped-60, quality+length filter,
  rename+uppercase — double as worked examples.

## Worked example

Input (2 reads):
```
@read1 sample
ACGTACGTNN
+
IIIIIIII##
@read2
ACGT
+
!!!!
```
Default convert → each `@`→`>`, quality dropped:
```
>read1 sample
ACGTACGTNN
>read2
ACGT
```
With `min_quality=20` (offset 33): read2's mean Phred is 0 (`!`=0) so it's
dropped; read1's mean ≈ 26.25 survives.
