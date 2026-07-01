## About this tool

This **BibTeX formatter and validator** tidies up your `.bib` bibliography
files. Paste your BibTeX source and it parses every entry, reports any syntax
error (unbalanced braces, a missing `=`, a truncated entry), then re-emits the
whole file in a clean, consistent style — one field per line, indented, with the
entry type and field names normalized to lowercase.

It runs entirely in your browser. Nothing is uploaded to a server, so even
unpublished or sensitive references stay on your machine.

### What it does

- **Validates** the syntax and points at the offending entry when something is wrong.
- **Pretty-prints** each entry with one field per line and configurable indentation.
- **Sorts entries** by cite key, or by entry type then key — handy for keeping a
  large bibliography in a stable, diff-friendly order.
- **Sorts fields** within each entry alphabetically, if you want a canonical field order.
- **Aligns the `=` signs** so the values line up in a neat column.
- **Lowercases** the entry type (`@ARTICLE` → `@article`) and field names, while
  leaving your **field values untouched** — `{braces}`, `"quotes"`, numbers and
  `#`-concatenated `@string` macros are all preserved exactly.

### Supported entry kinds

Normal entries (`@article`, `@book`, `@inproceedings`, `@misc`, …) plus
`@string` abbreviation definitions, `@preamble` blocks, and explicit
`@comment` blocks. Free text between entries is dropped, matching standard
BibTeX behaviour.

### Tips

- Use **Sort entries → by key** to keep your bibliography in a predictable order
  so version-control diffs stay small.
- Turn on **Align the = signs** for the most readable hand-edited `.bib` files.
- Set the **indent to 0** for a compact one-level layout, or up to 16 spaces for
  deeply indented output.

## FAQ

<details>
<summary>Will formatting change the text inside my field values?</summary>

No. Field **values are preserved byte-for-byte** — protective `{braces}`,
`"quoted"` values, bare numbers and `#`-concatenated `@string` macros all come
through unchanged. Only the entry type (`@ARTICLE` → `@article`, unless you
turn "Lowercase the @entry type" off) and the field names are normalized to
lowercase.

</details>

<details>
<summary>Where do @string, @preamble and @comment blocks go when I sort?</summary>

They stay at the **front of the file**, keeping their original relative order.
Sorting (`key` or `type-key`) only reorders the normal entries among
themselves, so abbreviations defined in `@string` are still declared before the
entries that use them.

</details>

<details>
<summary>Why am I getting a "duplicate cite key" error?</summary>

Duplicate-key checking is **on by default** because two entries with the same
key silently shadow each other in LaTeX. The error names the repeated key so
you can rename one of them; if the duplicates are intentional, untick
"Error on duplicate cite keys" and the file will format anyway.

</details>

<details>
<summary>Why did loose text between my entries disappear?</summary>

BibTeX itself treats anything outside an `@entry{...}` as an implicit comment,
so the formatter drops free text between entries to produce a canonical file.
If you need a comment to survive formatting, wrap it in an explicit
`@comment{...}` block — those are kept.

</details>
