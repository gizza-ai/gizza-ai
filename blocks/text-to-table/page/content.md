# Text to Table

Convert delimited text into an aligned table for terminals, docs, issues, and chat.

## Inputs

- **Delimited text** — CSV, TSV, semicolon, pipe, space, or another one-character delimiter.
- **Output format** — `ascii` for a padded grid with borders, or `markdown` for a GitHub-style pipe table.
- **First row is header** — when disabled, generated `Column N` headers are used.
- **Delimiter** — `,`, `tab`, `semicolon`, `pipe`, `space`, or any single character.
- **Alignment** — left, right, or center padding.

Quoted CSV fields are parsed with Rust's CSV parser, so commas inside quotes stay inside a cell.
