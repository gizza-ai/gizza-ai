## About this tool

`ris-bibtex-converter` moves a bibliography between the two formats that citation software and LaTeX disagree about. RIS is what EndNote, Zotero, Mendeley, RefWorks, Scopus, Web of Science and PubMed hand you when you click *Export* — a flat list of two-letter tag lines from `TY  - JOUR` to `ER  - `. BibTeX is what `\cite{}` needs — `@article{key, field = {value}, ...}`. This tool converts either one into the other, in your browser, with no account and no upload.

Leave **Direction** on *Auto-detect* and the input is sniffed: a `TY  - ` line means RIS in and BibTeX out, an `@article{` line means the opposite. Force the direction when the input is unusual and you would rather see a parse error than a guess.

Both readers are real parsers, not regex sweeps. On the RIS side, wrapped continuation lines are folded back onto their tag, a record missing its `ER  - ` terminator is still closed by the next `TY`, and prose that happens to look like a tag (`In - depth analysis` on a wrapped line) stays prose. On the BibTeX side — the reader is shared with the sibling `.bib` tools, so both agree on what a file means — `{braced}`, `"quoted"` and bare values, brace-balanced nesting, `@string` macros with `#` concatenation, `@comment`/`@preamble` blocks and parenthesised entries all parse.

### Worked example

This RIS record, straight out of a reference manager:

```
TY  - JOUR
AU  - Shannon, C. E.
TI  - A Mathematical Theory of Communication
JO  - Bell System Technical Journal
PY  - 1948
VL  - 27
IS  - 3
SP  - 379
EP  - 423
DO  - 10.1002/j.1538-7305.1948.tb01338.x
ER  -
```

with the default settings gives:

```
@article{shannon1948mathematical,
  author = {Shannon, C. E.},
  title = {A Mathematical Theory of Communication},
  journal = {Bell System Technical Journal},
  volume = {27},
  number = {3},
  pages = {379--423},
  year = {1948},
  doi = {10.1002/j.1538-7305.1948.tb01338.x}
}
```

`JOUR` became `@article`, `SP`/`EP` were joined into a `pages` range with the en-dash `--` that BibTeX wants, and — because RIS has no cite key at all — one was invented from the first author, the year and the first significant title word.

Going the other way, this book entry:

```
@book{knuth1984,
  author    = {Knuth, Donald E.},
  title     = {The {TeX}book},
  publisher = {Addison-Wesley},
  address   = {Reading, MA},
  year      = {1984},
  isbn      = {0-201-13447-0}
}
```

gives:

```
TY  - BOOK
ID  - knuth1984
AU  - Knuth, Donald E.
TI  - The TeXbook
PY  - 1984
PB  - Addison-Wesley
CY  - Reading, MA
SN  - 0-201-13447-0
ER  -
```

The cite key survives as the `ID` tag, `address` became `CY`, the ISBN went into `SN` (RIS uses one tag for both ISBN and ISSN), and the protective braces around `{TeX}` were dropped because RIS is plain text, not LaTeX.

### What maps to what

Reference types, both ways: `JOUR`/`MGZN`/`NEWS` ↔ `@article`, `BOOK`/`SER` ↔ `@book`, `CHAP` ↔ `@incollection`, `CONF`/`CPAPER` ↔ `@inproceedings`, `THES` ↔ `@phdthesis` or `@mastersthesis` (the `M3` tag decides), `RPRT`/`GOVDOC`/`STAND` ↔ `@techreport`, `UNPB`/`MANSCPT` ↔ `@unpublished`, `COMP` ↔ `@software`, `ELEC`/`BLOG`/`WEB`/`DATA` ↔ `@misc`. Anything unrecognised becomes `@misc` / `GEN` rather than being dropped.

Fields: `AU`/`A1` ↔ `author`, `A2`/`A3`/`ED` ↔ `editor`, `TI`/`T1` ↔ `title`, `JO`/`JF`/`J1`/`J2` ↔ `journal`, `T2`/`BT` ↔ `booktitle` (or `series` for a book), `T3` ↔ `series`, `VL` ↔ `volume`, `IS` ↔ `number`, `SP` + `EP` ↔ `pages`, `PY`/`Y1`/`DA` ↔ `year` and `month`, `PB` ↔ `publisher` (`school` for a thesis, `institution` for a report), `CY`/`PP` ↔ `address`, `ET` ↔ `edition`, `M3` ↔ `type`, `DO` ↔ `doi`, `UR` ↔ `url`, `Y2` ↔ `urldate`, `SN` ↔ `isbn` or `issn`, `LA` ↔ `language`, `KW` ↔ `keywords`, `AB`/`N2` ↔ `abstract`, `N1` ↔ `note`.

### Options and limits

- **Direction** — *Auto-detect* reads the first format marker it finds. If a file somehow contains both, whichever appears first wins. *RIS → BibTeX* and *BibTeX → RIS* force it.
- **Cite key style** only applies when BibTeX is being written, because RIS records have no key. *author + year + title word* gives `shannon1948mathematical` (stop words such as *a*, *the*, *on*, *of* are skipped); *author + year* gives `shannon1948`; *Reuse the RIS ID tag* takes the record's `ID` when there is one and falls back to author+year+word when there is not; *Numbered* gives `ref1`, `ref2`, `ref3`. Accents are folded to ASCII, so `Erdős` keys as `erdos`. A key that would repeat gets a trailing `a`, `b`, `c` so no two entries in the output collide.
- **Keep the abstract** and **Keep the keywords** are on by default. Turn the abstract off for a compact bibliography — abstracts are long and no common citation style prints them. Every `KW` tag joins into one comma-separated `keywords` field, and a `keywords` field splits back into one `KW` per term on commas or semicolons.
- **Translate LaTeX markup** is on by default and does the right thing per direction. Into RIS it decodes accent macros and protective braces to real UTF-8 (`M\"uller` → `Müller`, `Erd{\H o}s` → `Erdős`, `{DNA}` → `DNA`). Into BibTeX it escapes the characters that would otherwise break a compile — `& % $ # _ { } ~ ^ \` — while leaving `url` and `doi` values untouched so links stay clickable. Turn it off to pass every value through byte-for-byte.
- **BibTeX field indent** is 0–16 spaces, default 2. It has no effect when RIS is being written: RIS has a fixed `TAG  - value` line shape.
- **Record order** — *As in the input*, by cite key (case-insensitive), by year (undated records last), or by reference type then key. With *Numbered* keys the numbering follows the sorted order, so `ref1` is always the first record you see.
- The input limit is 1,000,000 bytes per run. Split a larger export and convert it in parts.
- Nothing is invented beyond the cite key: no record is enriched from an external database, no DOI is resolved, no journal abbreviation is expanded. A field with no counterpart in the target format is dropped rather than guessed at — RIS `AD` (author address) and `DB` (database name) have no BibTeX equivalent, and BibTeX `crossref`/`annote` have no RIS tag.
- A round trip is lossy in one direction by design: RIS → BibTeX → RIS regenerates the `ID` from the cite key that was invented on the way out, so it will not match a key your reference manager assigned unless you used *Reuse the RIS ID tag*.

## FAQ

<details>
<summary>My reference manager's export has no cite keys — where do the BibTeX keys come from?</summary>

They are generated, because RIS genuinely has no cite-key field. The default builds one from the first author's family name, the year and the first significant word of the title (`shannon1948mathematical`), which is the convention most people already type by hand. If your export includes an `ID` tag — Zotero and EndNote often write one — choose *Reuse the RIS ID tag* to keep the identifier your library already uses. If you cite by number and do not care about readability, *Numbered* gives `ref1`, `ref2`. Repeats never collide: the second `shannon1948mathematical` becomes `shannon1948mathematicala`.

</details>

<details>
<summary>Why does my `.bib` output contain backslashes that were not in the RIS?</summary>

Because ten characters are special to LaTeX and would break the compile if written literally. A title like `Cost & Benefit: 50% of R_2` is emitted as `Cost \& Benefit: 50\% of R\_2`, which typesets as the original text. `url` and `doi` values are deliberately exempt so an underscore in a link is not mangled. Turn **Translate LaTeX markup** off if you want the values passed through verbatim — useful when the target is not LaTeX at all, for instance when you are feeding the `.bib` to a script.

</details>

<details>
<summary>Are accented characters and non-English names handled?</summary>

Yes, in both directions. Coming from BibTeX, accent macros are decoded to real UTF-8: `M\"uller` becomes `Müller`, `Erd{\H o}s` becomes `Erdős`, `\ss` becomes `ß`, and protective braces around acronyms disappear so `{DNA}` stays `DNA`. Going to BibTeX, UTF-8 is passed through as-is (every modern engine reads UTF-8 `.bib` files), and only the LaTeX-special punctuation is escaped. Cite keys are a separate matter — those are folded to plain ASCII, so `Erdős, Pál` keys as `erdos1959…`, because BibTeX keys with non-ASCII bytes are a portability trap.

</details>

<details>
<summary>What happens to a reference type or field the other format does not have?</summary>

Types fall back rather than fail: an unrecognised RIS type becomes `@misc` and an unrecognised BibTeX type becomes `GEN`, so the record is never silently dropped. Fields with no counterpart are dropped — RIS `AD`, `DB` and `DP`, BibTeX `crossref` and `annote` — and a few are merged because the target format merges them: RIS `SN` carries both ISBN and ISSN, so which BibTeX field it becomes depends on the entry type. A thesis keeps its `school` as `PB` plus `M3  - PhD thesis`, and that `M3` is what tells the converter to pick `@phdthesis` versus `@mastersthesis` on the way back.

</details>

<details>
<summary>Can I convert a whole library at once, and is it uploaded anywhere?</summary>

Paste as many records as you like up to 1,000,000 bytes — a few thousand references — and use **Record order** to sort the result by key, year or type in the same pass. Nothing is uploaded: the same Rust core is compiled to WebAssembly for this page and runs entirely in your browser, and the identical code runs locally in the CLI. No server sees the bibliography and no external database is consulted.

</details>
