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
