## About this tool

`bibtex-to-csv` turns a BibTeX bibliography into a CSV table: one row per `@type{key, ...}` entry, one column per field. It is for the moments when a `.bib` file has to leave LaTeX — auditing a reference list in a spreadsheet, importing into a reference manager or a database, checking for duplicate DOIs, or handing a publication list to someone who does not use LaTeX.

The parser is a real BibTeX parser, not a regex sweep. It reads `{braced}`, `"quoted"` and bare-number values, brace-balanced nesting, `@string` macros with `#` concatenation, `@comment` / `@preamble` blocks, parenthesised entries (`@article(key, ...)`), values that wrap across lines, and free text between entries. LaTeX accent macros are decoded to real UTF-8 by default, so `\'{e}` becomes `é`, `{\H o}` becomes `ő`, `\ss` becomes `ß`, `---` becomes an em dash, and protective braces disappear so `{DNA}` stays `DNA`. Output is quoted per RFC 4180: a cell is wrapped in quotes only when it contains the delimiter, a double quote, or a line break, and internal quotes are doubled.

### Worked example

This BibTeX:

```
@article{curie1898,
  title   = {Sur une substance nouvelle radio-active},
  author  = {Curie, Marie and Curie, Pierre},
  journal = {Comptes Rendus},
  year    = {1898},
  volume  = {127},
  pages   = {175--178}
}
```

with the default settings gives:

```
type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url
article,curie1898,Sur une substance nouvelle radio-active,"Curie, Marie and Curie, Pierre",1898,Comptes Rendus,,127,,175–178,,,,,
```

Every standard column is present even when the entry has no such field, so rows from different entry types stay aligned. The author cell is quoted because it contains a comma; `175--178` was decoded to an en dash.

Switch **Columns** to *Custom*, list `key,author,year`, set **Author name format** to *Last, First* and **Author separator** to *Semicolon*, and the same input gives:

```
key,author,year
curie1898,"Curie, Marie; Curie, Pierre",1898
```

### Options and limits

- **Columns** — *Standard* is the fixed set `type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url`. *All fields* emits `type,key` then every field name that appears anywhere in the file, alphabetised. *Custom* emits exactly the names you list, in your order.
- **Custom column names** are matched case-insensitively against field names; `type` and `key` are accepted as virtual columns, and a name no entry uses becomes an empty column. Up to 200 columns.
- **Delimiter** — comma, semicolon, tab or pipe. Semicolon is the one to pick when the CSV is destined for Excel in a locale that uses a comma as the decimal mark.
- **Header row** can be turned off when appending to an existing CSV.
- **Decode LaTeX accents and braces** is on by default. Turn it off to keep the source spelling byte-for-byte, which is what you want if the CSV is going back into LaTeX later.
- **Author name format** and **Author separator** only touch the `author`, `editor` and `translator` fields. Lowercase particles (`van`, `von`, `de`) stay with the last name, a `Last, Jr, First` suffix stays glued to the last name, and a brace-protected corporate name such as `{The MIT Press}` is never split.
- **Expand @string macros** resolves macro names and `#` concatenation. Off, the unresolved macro name is written into the cell instead.
- **Row order** — file order, cite key, year (entries with no parseable year go last), or entry type then key.
- **Add UTF-8 BOM for Excel** prepends a byte-order mark. It is off by default because parsers that do not strip a BOM will report the first column name as `﻿type`.
- The input limit is 1,000,000 bytes per run. Split a larger `.bib` file and convert it in parts.
- Fields are not renamed, merged or validated: `journal` and `booktitle` stay separate columns (an `@inproceedings` entry can carry both), and no entry is enriched from an external database.

## FAQ

<details>
<summary>Why is my author column one cell instead of several?</summary>

BibTeX stores all authors in one field, so this tool keeps one `author` cell per entry — splitting into `author_1`, `author_2`, … would give every row a different width. Choose an **Author separator** of *Semicolon* or *Pipe* if you want to split the cell later in a spreadsheet, because those survive a comma-delimited CSV without ambiguity. Pair it with the *Last, First* format when you need a consistent spelling.

</details>

<details>
<summary>My accented characters look wrong in Excel — what do I change?</summary>

Two separate settings. Turn on **Add UTF-8 BOM for Excel** so Excel recognises the file as UTF-8 rather than guessing a legacy code page. If the whole row also lands in one column, switch the **Delimiter** to *Semicolon*, which is what Excel expects in locales that use a comma as the decimal mark. Leave the BOM off for pandas, R, or anything that reads UTF-8 directly.

</details>

<details>
<summary>What happens to entries that are missing a field?</summary>

The cell is empty; the row is never shortened and the entry is never dropped. That is why *Standard* mode always emits the same 15 columns. If you want to see only the fields that actually exist in your file, switch **Columns** to *All fields found in the file* — it takes the union of every field name across all entries.

</details>

<details>
<summary>Can it handle @string macros, `#` concatenation and `@comment` blocks?</summary>

Yes. `@string{jcp = "J. Chem. Phys."}` followed by `journal = jcp # { (Letters)}` produces `J. Chem. Phys. (Letters)` with **Expand @string macros** on, and the raw `jcp (Letters)` with it off. `@comment` and `@preamble` blocks, and any prose between entries, are parsed and skipped rather than turned into rows. A genuinely broken entry is a hard error naming the entry and the character position, so nothing is silently half-converted.

</details>

<details>
<summary>Is my bibliography uploaded anywhere?</summary>

No. The same Rust core is compiled to WebAssembly for this page and runs in your browser, and the identical code runs locally in the CLI. Nothing is sent to a server, and no external database is consulted.

</details>
