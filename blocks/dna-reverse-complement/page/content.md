## What this tool does

Paste a **DNA or RNA sequence** and get its **reverse complement** — the opposite
strand, read in the usual 5'→3' direction. Each base is swapped for its partner
(A↔T, C↔G, and U pairs with A) and the order is flipped. You can also ask for the
**complement only** (no flip) or the **reverse only** (no complementing).

Everything runs locally in your browser tab as WebAssembly: your sequence is never
uploaded, the tool works offline, and there is no sign-up.

## The IUPAC nucleotide alphabet

Ambiguity codes are handled in full, using the standard IUPAC pairing. Note that
**S** and **W** are their own complements — a classic source of wrong answers in
hand-written tables.

| Code | Means | Complement |
| --- | --- | --- |
| `A` | adenine | `T` (or `U`) |
| `C` | cytosine | `G` |
| `G` | guanine | `C` |
| `T` / `U` | thymine / uracil | `A` |
| `R` | A or G (purine) | `Y` |
| `Y` | C or T (pyrimidine) | `R` |
| `S` | G or C | `S` |
| `W` | A or T | `W` |
| `K` | G or T (keto) | `M` |
| `M` | A or C (amino) | `K` |
| `B` | C, G or T (not A) | `V` |
| `D` | A, G or T (not C) | `H` |
| `H` | A, C or T (not G) | `D` |
| `V` | A, C or G (not T) | `B` |
| `N` | any base | `N` |
| `-` / `.` | alignment gap | unchanged |

## Options

| Option | What it does |
| --- | --- |
| **Operation** | **Reverse complement** (default) complements and flips — the opposite strand. **Complement only** swaps bases in place. **Reverse only** flips the order without swapping. |
| **Output alphabet** | **Auto** (default) keeps the input's alphabet: RNA out if the input has `U` and no `T`, otherwise DNA. **DNA** writes `U` as `T`; **RNA** writes `T` as `U`. |
| **Preserve case** | On by default, so lower-case regions (often used to mark repeats, primers or UTRs) stay marked. Turn it off to uppercase the whole output. |
| **Wrap width** | Split the output into fixed-width lines. `0` = one line per sequence; `60` is the usual FASTA convention. Maximum `200`. |
| **Unrecognised characters** | Anything that is not a base, IUPAC code or gap. **Report an error** (default) names the character and its position; **Remove them** strips them; **Pass them through** leaves them in place. |
| **Append length / GC summary** | Adds `#` comment lines with the record count, length, GC content, ambiguous-code count and gap count. |

## Worked examples

### A plain sequence

Input:

```
ATGGCCATTGTAATGGGCCGC
```

Reverse complement:

```
GCGGCCCATTACAATGGCCAT
```

With **Operation** set to *Complement only*, the same input gives the partner base
at every position without flipping the order:

```
TACCGGTAACATTACCCGGCG
```

### Ambiguity codes

Input `ACGTRYSWKMBDHVN` reverse-complements to:

```
NBDHVKMWSRYACGT
```

### FASTA in, FASTA out

Input (two records) with **Wrap width** `10`:

```
>seq1 forward primer
ATGGCCATTGTAATGGGCCGC
>seq2
TTACGGATCC
```

Output — each record is transformed on its own and its header is kept:

```
>seq1 forward primer
GCGGCCCATT
ACAATGGCCA
T
>seq2
GGATCCGTAA
```

### A numbered paste

Sequence copied out of a numbered listing, with **Unrecognised characters** set to
*Remove them* and the summary switched on:

```
1 atggccatt 10 gtaatgggc
```

gives

```
gcccattacaatggccat

# sequences: 1
# length: 18
# gc_content: 50.00%
# ambiguous: 0
# gaps: 0
```

Note that the lower case is preserved and the position numbers are gone.

## Limits and edge cases

- Input is capped at **1,000,000 characters** — roughly a bacterial chromosome.
  Split larger genomes before pasting; a browser tab is bounded by your device's
  memory.
- Spaces, tabs and line breaks **between** bases are always ignored, so a wrapped
  or column-formatted paste works as-is. The output's line breaks come only from
  the wrap width.
- **Wrap width** must be `0`–`200`.
- Alignment gaps `-` and `.` are kept and map to themselves, so a gapped alignment
  row round-trips.
- With **Auto** alphabet, a sequence containing both `T` and `U` is treated as DNA.
  Force the alphabet if that is not what you want.
- FASTA is detected from any line starting with `>`. Sequence appearing before the
  first header is still transformed, as a record with no header.
- Reverse-complementing twice returns the original sequence — a quick way to check
  a result.
- The GC percentage counts only unambiguous bases (`A`, `C`, `G`, `T`, `U`);
  ambiguity codes and gaps are excluded from both the numerator and the divisor.

## FAQ

<details>
<summary>What is the difference between the reverse complement and the complement?</summary>

The **complement** swaps each base for its pairing partner but leaves the order
alone, so position 1 stays at position 1. The **reverse complement** also flips the
sequence end-to-end, which is what you want when you read the opposite strand in
the standard 5'→3' direction — for designing a reverse primer, for example, or for
finding an open reading frame on the minus strand.

</details>

<details>
<summary>How are IUPAC ambiguity codes complemented?</summary>

They follow the IUPAC convention, which you can read straight off the code table
above: `R`↔`Y`, `K`↔`M`, `B`↔`V`, `D`↔`H`, while `S`, `W` and `N` are their own
complements. `S` means "G or C", and the complement of "G or C" is "C or G" — the
same set — which is why it is unchanged. The same reasoning applies to `W`
("A or T").

</details>

<details>
<summary>Can I paste RNA, and what comes out?</summary>

Yes. With the **Auto** output alphabet (the default), a sequence containing `U` and
no `T` is treated as RNA and the result is written with `U`. Set the output
alphabet to **DNA** to get a `T`-based result from RNA input (handy for going back
to a cDNA sequence), or to **RNA** to transcribe a DNA result into `U`.

</details>

<details>
<summary>Does it handle multi-record FASTA files?</summary>

Yes. Any line starting with `>` begins a new record. Every record is transformed
independently, its header line is copied through unchanged, and the sequence lines
of a record are joined before transforming — so a file wrapped at 60 or 70
characters gives the right answer regardless of where the line breaks fall.

</details>

<details>
<summary>Why did it reject my sequence, and how do I fix it?</summary>

By default anything that is not a base, an IUPAC code, or a gap is an error, and
the message names the offending character and its position. That usually means the
paste carried along position numbers, a stop-codon `*`, or stray punctuation. Set
**Unrecognised characters** to *Remove them* to strip that noise, or to *Pass them
through* if the extra symbols are meaningful annotation you want to keep.

</details>

<details>
<summary>Is my sequence uploaded anywhere?</summary>

No. The transform is compiled to WebAssembly and runs inside your browser tab.
Nothing is sent to a server, so the page also works with the network switched off
once it has loaded — which matters for unpublished or patient-derived sequence.

</details>
