## What this tool does

Paste a **FASTA** or multi-FASTA file and get a **spreadsheet-ready table** back —
one row per record. By default each row carries the record's `id`, its
`description`, the full `sequence` and its `length`; wrapped sequence lines are
joined back into a single field first.

Nothing is uploaded. The parsing runs locally in your browser using WebAssembly,
so it works offline and needs no sign-up.

## Columns

| Column | When | What it holds |
| --- | --- | --- |
| `id` | always | The header text up to the first space (or the whole header, see below). |
| `description` | header handling = *split* | Whatever follows that first space. |
| `sequence` | **Sequence column** on | The record's sequence with all wrapped lines joined. |
| `length` | **Length column** on | Number of sequence characters — gaps and ambiguity codes such as `N` count. |
| `gc_percent` | **GC content** on | `(G+C) / (A+C+G+T) × 100`, two decimals, case-insensitive; `N` and gaps are excluded from both sides. |
| `a_count` … `other_count` | **Base counts** on | Case-insensitive counts of `A`, `C`, `G`, `T` and everything else. |

## Options

| Option | What it does |
| --- | --- |
| **Delimiter** | `Comma` for `.csv`, `Tab` for `.tsv`, or `Semicolon`/`Pipe` when your spreadsheet uses a comma as its decimal separator. |
| **Header handling** | *Split into id + description* (default), *Id only* (drops the description column), or *Whole header line as id* (keeps `>gi\|123\|ref\|NM_000.1 Homo sapiens` intact in one cell). |
| **Write a header row** | On by default so the table opens with named columns. Turn it off for a bare data table. |
| **Sequence / Length columns** | Both on by default. Turn the sequence off for a names-and-metrics table. |
| **GC content / Base counts** | Add the extra metric columns described above. |
| **Uppercase sequence** | Normalises `acgt` → `ACGT` in the sequence column only. |
| **Drop duplicate sequences** | Keeps the first record of each identical sequence (compared case-insensitively). |

## Worked example

Input:

```
>seq1 first sequence
ACGTACGTNN
>seq2
acgt
```

Default output — comma delimiter, header row, id split from description:

```
id,description,sequence,length
seq1,first sequence,ACGTACGTNN,10
seq2,,acgt,4
```

`seq2` has no description, so that cell is empty. Now switch **GC content** and
**Base counts** on and the same input gives:

```
id,description,sequence,length,gc_percent,a_count,c_count,g_count,t_count,other_count
seq1,first sequence,ACGTACGTNN,10,50.00,2,2,2,2,2
seq2,,acgt,4,50.00,1,1,1,1,0
```

`seq1` is 10 characters long, but its two `N`s land in `other_count` and are left
out of the GC calculation — 4 of its 8 unambiguous bases are `G` or `C`, so
`gc_percent` is `50.00`.

## Limits and edge cases

- The input must contain at least one `>` header line. Sequence data appearing
  before the first header is rejected with the offending line number.
- Up to **50,000 records** per conversion. Split a larger file and convert it in
  parts; very large inputs are also bounded by your device's memory.
- Fields are quoted per **RFC 4180**: any value containing the delimiter, a
  double quote or a line break is wrapped in `"` and its inner quotes are
  doubled. Descriptions with commas are therefore safe in CSV mode.
- Blank lines are ignored, `CRLF` line endings are normalised, and a header with
  no sequence after it yields an empty `sequence` cell with `length` `0`.
- `length`, `gc_percent` and the base counts are computed from the original
  sequence, so **Uppercase sequence** never changes them.

## FAQ

<details>
<summary>How does the tool decide what is the id and what is the description?</summary>

With the default *Split into id + description* setting, everything up to the first
whitespace character in the header becomes `id` and the remainder becomes
`description`. `>seq1 first sequence` therefore yields `seq1` and
`first sequence`. If your identifiers contain spaces you care about, choose
*Whole header line as id* to keep the header in a single cell.

</details>

<details>
<summary>Can I get a TSV instead of a CSV?</summary>

Yes — set **Delimiter** to `Tab`. The columns and options are identical; only the
separator changes, and commas inside descriptions no longer need quoting. Save the
result with a `.tsv` extension.

</details>

<details>
<summary>What does the length column count?</summary>

Every character of the joined sequence, including ambiguity codes like `N` and
alignment gaps like `-`. That matches how most FASTA tooling reports length. If
you need the unambiguous base total instead, switch on **Base counts** and add
`a_count + c_count + g_count + t_count`.

</details>

<details>
<summary>Does it work with protein FASTA, not just DNA?</summary>

Yes. Parsing, the id/description split, `length` and the CSV quoting are all
sequence-agnostic. The `gc_percent` column is only meaningful for nucleotide
sequences, and for protein input the base counts put nearly every residue into
`other_count`, so leave those two options off.

</details>

<details>
<summary>Is my sequence data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using WebAssembly. Your sequences
never leave your device, and the page keeps working offline once it has loaded.

</details>
