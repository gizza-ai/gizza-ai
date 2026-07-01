# Text to Table

Convert delimited text into an aligned table for terminals, docs, issues, and chat.

## Inputs

- **Delimited text** — CSV, TSV, semicolon, pipe, space, or another one-character delimiter.
- **Output format** — `ascii` for a padded grid with borders, or `markdown` for a GitHub-style pipe table.
- **First row is header** — when disabled, generated `Column N` headers are used.
- **Delimiter** — `,`, `tab`, `semicolon`, `pipe`, `space`, or any single character.
- **Alignment** — left, right, or center padding.

Quoted CSV fields are parsed with Rust's CSV parser, so commas inside quotes stay inside a cell.

## FAQ

<details>
<summary>What's the difference between the ascii and markdown formats?</summary>

**ascii** draws a padded grid with `+---+` box borders — ideal for a terminal,
a log or a plain-text README. **markdown** emits a GitHub-style pipe table
(`| col | col |` with a `---` separator row) that renders as a real table in
issues, PRs, docs and chat.

</details>

<details>
<summary>Do my column alignments carry into the Markdown table?</summary>

Yes. The left/right/center choice sets the separator-row markers in Markdown
(`:--`, `--:`, `:-:`), so viewers that support alignment render the columns
accordingly. In ascii output the same setting controls how cell text is padded
within each column.

</details>

<details>
<summary>How are commas and pipes inside a cell handled?</summary>

Input is parsed as real CSV, so a comma inside `"quoted, text"` stays in one
cell. When you export to Markdown, any literal `|` or `\` in a value is escaped
(`\|`, `\\`) and embedded newlines become spaces, so a stray character can't
break the table layout.

</details>

<details>
<summary>Can I convert tab- or pipe-separated data, not just CSV?</summary>

Yes — set the delimiter to `tab`, `semicolon`, `pipe`, `space`, or type any
single character. If your data has no header row, untick "First row is header"
and the tool labels the columns `Column 1`, `Column 2`, and so on.

</details>
