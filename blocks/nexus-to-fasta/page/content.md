## What this tool does

Convert a **NEXUS** sequence-alignment matrix into standard **FASTA**. Paste the
file text, including `#NEXUS` and its `begin data;` or `begin characters;` block,
and the tool extracts the `matrix` command into one `>name` FASTA record per
taxon. It runs locally in your browser, so the alignment is not uploaded.

NEXUS is flexible: comments can appear almost anywhere, taxon labels may be quoted,
DATA and CHARACTERS blocks are both common, and matrices may be sequential or
interleaved. This converter handles those everyday forms and reports dimension
mismatches clearly.

## Supported NEXUS features

| Feature | Handling |
| --- | --- |
| `DATA` and `CHARACTERS` blocks | The first matching block is used; TAXA/TREES/ASSUMPTIONS are ignored except for `taxlabels`. |
| `dimensions ntax=… nchar=…` | Used to validate taxon count and site count. |
| `format interleave` | Honoured by Auto-detect; you can also force sequential or interleaved. |
| `format gap=…` | The declared gap symbol is preserved, or stripped when **Strip gaps** is on. |
| `format matchchar=.` | Expanded from the first taxon's residue by default. |
| `format labels=no` | Rows are paired with labels from the TAXA block's `taxlabels` command. |
| Bracketed `[comments]` | Removed before parsing, including nested comments. |
| Quoted taxon labels | Single-quoted labels with spaces are preserved. |

## Options

| Option | What it does |
| --- | --- |
| **Matrix layout** | `Auto-detect` (default), `Sequential`, or `Interleaved`. Auto honours the `interleave` flag and otherwise chooses the parse that matches `nchar`. |
| **FASTA line width** | Wrap each sequence at this many characters. Default `60`; `0` writes one long line per sequence. |
| **Residue case** | Keep, uppercase, or lowercase sequence residues. Taxon labels are not case-normalised. |
| **Strip gaps** | Remove the declared `gap=` symbol plus common `-` and `.` gap marks. |
| **Expand matchchar** | Replace a declared `matchchar` (usually `.`) with the first taxon's residue at the same site. On by default. |
| **Unquoted underscores become spaces** | Apply the NEXUS convention where `Homo_sapiens` in an unquoted label means `Homo sapiens`. Off by default because many FASTA workflows prefer underscore-stable headers. |
| **Tolerant** | Convert anyway when `ntax` or `nchar` checks fail. Off by default. |

## Worked example

A simple DATA block:

```
#NEXUS
begin data;
  dimensions ntax=2 nchar=8;
  format datatype=dna gap=-;
  matrix
    Alpha  ACGTACGT
    Beta   ACGTTCGT
  ;
end;
```

With **FASTA line width** set to `0`, the output is:

```
>Alpha
ACGTACGT
>Beta
ACGTTCGT
```

An interleaved CHARACTERS block with `matchchar=.` expands the dots from the first
taxon when **Expand matchchar** is on:

```
#NEXUS
begin characters;
  dimensions ntax=2 nchar=8;
  format datatype=dna gap=- matchchar=. interleave;
  matrix
    Alpha  ACGT
    Beta   ....

    Alpha  TGCA
    Beta   ....
  ;
end;
```

```
>Alpha
ACGTTGCA
>Beta
ACGTTGCA
```

## Limits and edge cases

- The document must start with `#NEXUS` after leading whitespace.
- A `begin data;` or `begin characters;` block with a `matrix` command is required.
- Strict mode checks the number of parsed taxa against `ntax` and each sequence's
  site count against `nchar`. The error names the offending taxon when possible.
- `nchar` counts NEXUS state sets such as `(01)` or `{AC}` as one site; wrapping
  will not split the bracketed set.
- The converter does not output trees from a TREES block. It only converts the
  sequence matrix.
- FASTA has no datatype field, so `datatype=dna`, `protein`, and `standard` are
  treated as alignment text rather than changing the output format.
- FASTA line width is capped at 1000 characters per line.

## FAQ

<details>
<summary>Does this convert TREE blocks too?</summary>

No. It only extracts the sequence matrix from a `DATA` or `CHARACTERS` block and
writes FASTA. NEXUS tree definitions are a different data shape and are ignored.

</details>

<details>
<summary>Why does the output differ when Expand matchchar is on?</summary>

In NEXUS, a declared `matchchar` such as `.` means "use the same state as the
first taxon at this site." Expanding it writes the actual residue into the FASTA
record, which is usually what downstream FASTA tools expect. Turn the option off
when you want literal dots preserved.

</details>

<details>
<summary>How are spaces in taxon names handled?</summary>

Single-quoted labels such as `'Homo sapiens'` keep the space. For unquoted labels,
NEXUS treats underscores as spaces; this tool can apply that convention with
**Unquoted underscores become spaces**, but keeps underscores by default for safer
FASTA headers.

</details>

<details>
<summary>Can I convert interleaved NEXUS files?</summary>

Yes. Auto-detect honours `format interleave` and also tries the interleaved parse
when no flag is present. Force **Matrix layout** to `Interleaved` if the file is
malformed and you still want that interpretation.

</details>

<details>
<summary>What does a dimension mismatch mean?</summary>

The `dimensions` command declares how many taxa and sites the matrix should have.
A mismatch usually means the file is truncated, a wrong layout was forced, or the
matrix contains labels/no-labels in a different shape than declared. Use
**Tolerant** only when you want the partial sequences anyway.

</details>
