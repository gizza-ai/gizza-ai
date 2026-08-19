## What this tool does

Convert a **PHYLIP** multiple-sequence alignment into standard **FASTA**, right in
your browser. Paste the alignment — the count header plus the body — and the tool
works out whether it is sequential or interleaved, whether the taxon names are
strict 10-column or relaxed whitespace-delimited, and writes one `>name` record
per taxon. Nothing is uploaded: it runs locally, works offline, and needs no
sign-up.

## The two PHYLIP layouts

Every PHYLIP file opens with a count header — the number of taxa and the number
of aligned sites, e.g. `3 12`. What follows comes in one of two shapes.

| Layout | Body shape |
| --- | --- |
| **Sequential** | Each taxon's whole sequence is written out (possibly wrapped over several lines) before the next taxon begins. |
| **Interleaved** | The first block holds every taxon's name plus the first chunk of its sequence; each later block appends the next chunk, in the same taxon order. |

Taxon names come in two shapes too. **Strict** PHYLIP puts the name in columns
1–10 and starts the sequence at column 11, with no separator required — which is
why classic PHYLIP truncates names at ten characters. **Relaxed** PHYLIP (the
RAxML / PhyML convention) uses the first whitespace-delimited word as the name, so
names can be any length.

Leave both selectors on **Auto-detect** and the tool tries each candidate parse
and keeps the one whose sequences match the site count in the header. Force a
specific layout or name style when you know your file and want the parse pinned.

## Options

| Option | What it does |
| --- | --- |
| **Body layout** | `Auto-detect` (default), `Sequential`, or `Interleaved`. |
| **Taxon name style** | `Auto-detect` (default), `Strict` (columns 1–10), or `Relaxed` (first word). |
| **FASTA line width** | Wrap each sequence at this many characters. Default `60`, the conventional FASTA width; `0` writes one long line per sequence. |
| **Uppercase residues** | Normalise the sequence case (`acgt` → `ACGT`). |
| **Strip gaps** | Remove the alignment gap characters `-` and `.`, turning the aligned FASTA into unaligned sequences. Off by default, so gaps survive untouched. |
| **Tolerant** | Convert anyway when the file disagrees with its own header — wrong taxon count, wrong sequence length, or unexpected residue characters. Off by default, so mismatches are reported instead. |

## Worked example

An interleaved alignment of three taxa over twelve sites:

```
3 12
Alpha     ACGT
Beta      ACGA
Gamma     TCGA

ACGTACGT
ACGTACGT
ACGTACGT
```

With **FASTA line width** set to `0`, each taxon's two chunks are joined into one
record:

```
>Alpha
ACGTACGTACGT
>Beta
ACGAACGTACGT
>Gamma
TCGAACGTACGT
```

The same alignment written sequentially — `Alpha`'s twelve sites, then `Beta`'s,
then `Gamma`'s — produces exactly the same FASTA, because Auto-detect recognises
both bodies.

Now a gapped alignment with **Strip gaps** switched on:

```
2 10
Alpha     AC--GTAC-T
Beta      ACGTGT--CT
```

```
>Alpha
ACGTACT
>Beta
ACGTGTCT
```

## Limits and edge cases

- The first non-blank line must be the count header, two whole numbers such as
  `3 12`. A trailing `I` or `S` on that line is read as a layout hint.
- By default every taxon's sequence must be exactly as long as the declared site
  count, and every residue must be a letter, a digit, or one of `-` `.` `?` `*`
  `~`. A mismatch names the taxon and reports both numbers; switch on **Tolerant**
  to convert regardless.
- With **Strict** name style forced, a name longer than ten characters is cut at
  column 10 and the remainder is read as sequence — that is the format's own rule,
  not a bug. Use `Relaxed` or `Auto-detect` for long names.
- Interleaved files that repeat the taxon name in every block are handled: the
  repeated name is detected and dropped rather than pasted into the sequence.
- Gap characters are preserved exactly unless **Strip gaps** is on, so the
  alignment columns stay intact for downstream tools.
- FASTA line width is capped at 1000 characters per line.
- Everything runs in the browser tab, so very large alignments are bounded by your
  device's memory; split huge files before pasting.

## FAQ

<details>
<summary>Do I need to know whether my file is sequential or interleaved?</summary>

No. Leave **Body layout** on `Auto-detect` and the tool parses the file both ways
and keeps the reading whose sequence lengths match the site count in the header.
Force one of the two only if you want the parse pinned, or if a badly-formed file
is being read the wrong way.

</details>

<details>
<summary>Why did my long taxon names get cut to ten characters?</summary>

That happens when the **Taxon name style** is forced to `Strict`, where the name
is defined as columns 1–10 of the line. Modern relaxed PHYLIP writers (RAxML,
PhyML) allow longer names separated by whitespace — pick `Relaxed`, or leave the
selector on `Auto-detect`, which prefers the relaxed reading whenever it produces
sequences of the declared length.

</details>

<details>
<summary>Are gap characters preserved?</summary>

Yes, by default. `-` and `.` pass through unchanged, so the FASTA output is still
a valid alignment with the same columns. Turn on **Strip gaps** when you want
unaligned sequences instead — for example to feed a BLAST search or re-align with
different settings.

</details>

<details>
<summary>What does the site-count error mean?</summary>

The header declares how many aligned sites each sequence should have. If a taxon
comes out shorter or longer, the file is usually truncated, mis-wrapped, or in a
layout that does not match the forced selector. The message names the taxon and
gives both the parsed and the declared count, so you can find the offending block.
**Tolerant** converts anyway if you just want the sequences out.

</details>

<details>
<summary>Is my alignment uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using WebAssembly. Your sequences
never leave your device, and the page keeps working offline once it has loaded.

</details>
