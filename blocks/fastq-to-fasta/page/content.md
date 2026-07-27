## What this tool does

Convert **FASTQ** sequencing reads to **FASTA** instantly, right in your browser.
Each FASTQ read is four lines — a header, the sequence, a `+` separator, and the
per-base quality string. This tool drops the `+` and quality lines, rewrites the
`@` header as `>`, and (optionally) filters or reformats the reads. Nothing is
uploaded: it runs locally, works offline, and needs no sign-up.

Paste one or more reads, tune the options, and copy the FASTA out.

## FASTQ vs FASTA

| Format | Lines per record | Carries quality? |
| --- | --- | --- |
| **FASTQ** | 4 — `@id`, sequence, `+`, quality | Yes (per-base Phred) |
| **FASTA** | 2 — `>id`, sequence | No |

## Options

| Option | What it does |
| --- | --- |
| **Min read length** | Drop reads shorter than N bases. `0` = keep all. |
| **Min mean quality** | Drop reads whose *mean* Phred quality is below the threshold. `0` = no quality filter. |
| **Quality offset** | How to decode quality characters: **Phred+33** (Sanger / Illumina 1.8+, the modern default) or **Phred+64** (old Illumina 1.3–1.5). Only matters when a quality filter is active. |
| **Wrap width** | Split each output sequence into fixed-width lines (e.g. `60` or `70`). `0` = one line per sequence. |
| **Rename headers** | Replace every header with a sequential number (`>1`, `>2`, …). |
| **Discard reads with N** | Drop any read whose sequence contains an ambiguous `N` base. |
| **Uppercase** | Uppercase the sequence bases (`acgt` → `ACGT`). |

## Worked example

Input (two reads):

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

Default conversion (no filters) → the quality lines are gone and `@` becomes `>`:

```
>read1 sample
ACGTACGTNN
>read2
ACGT
```

Now set **Min mean quality** to `20` (offset Phred+33). The `!` character is
Phred 0, so `read2` (mean 0) is dropped, while `read1` (mean ≈ 26) survives:

```
>read1 sample
ACGTACGTNN
```

## Limits and edge cases

- Input must be well-formed FASTQ: a multiple of 4 lines, each header starting
  with `@`, each third line starting with `+`, and the quality string the same
  length as its sequence. A mismatch reports which read failed and why.
- The quality filter needs the right **quality offset** — a quality character
  below the offset (a negative Phred) is reported as a likely wrong-offset error.
- Wrap width is capped at 1000 characters per line.
- Everything runs in the browser tab, so extremely large files are bounded by
  your device's memory; split very large FASTQ files before pasting.

## FAQ

<details>
<summary>What is the difference between FASTQ and FASTA?</summary>

FASTQ stores four lines per read — a header, the sequence, a `+` separator, and a
per-base quality string — while FASTA stores just a header and the sequence. This
tool removes the quality information and keeps the sequence, converting `@` headers
into `>` headers.

</details>

<details>
<summary>Which quality offset should I pick?</summary>

Use **Phred+33** for anything modern (Sanger and Illumina 1.8 or newer) — this is
the default. Use **Phred+64** only for legacy Illumina 1.3–1.5 data. The offset
only affects the mean-quality filter; if you leave that filter off, the setting is
ignored.

</details>

<details>
<summary>How is the mean quality calculated?</summary>

Each quality character is decoded to a Phred score as `ord(char) - offset`, and the
scores are averaged across the read. A read is dropped when that average is below
your **Min mean quality** threshold. `!` decodes to 0 (offset 33), `I` to 40.

</details>

<details>
<summary>Does the tool discard reads containing N?</summary>

Not by default — N-containing reads are kept so the conversion is loss-free. Turn
on **Discard reads with N** to drop any read whose sequence contains an ambiguous
`N` (or `n`) base.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using WebAssembly. Your reads
never leave your device, and the page keeps working offline once it has loaded.

</details>
