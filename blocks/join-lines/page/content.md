## About this tool

**Join Lines** merges a list of lines into a single line — paste your lines and
choose what goes between them.

- **Separator** — the text placed between lines. Type a comma, a space, a pipe
  `|`, or anything you like. Use `\t` for a tab and `\n` for a newline, and `\\`
  for a literal backslash. Leave it blank to concatenate the lines with nothing
  in between.
- **Trim each line** — strip leading and trailing whitespace from every line
  before joining.
- **Remove blank lines** — drop empty or whitespace-only lines instead of
  joining them.
- **Prefix each line** / **Suffix each line** — wrap every line, for example with
  quotes to build a comma-separated list or parentheses for a SQL `IN (…)` clause.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Turning a column of values into a comma-separated list for a spreadsheet, CSV
  or config file.
- Building a SQL `IN ('a', 'b', 'c')` clause from a pasted list of IDs.
- Collapsing a multi-line snippet into one line for a single-line field or log.
- Joining words or tokens with a custom delimiter such as a tab or pipe.

## FAQ

<details>
<summary>How do I use a tab (or a literal backslash) as the separator?</summary>

Type the escape into the separator box: `\t` becomes a tab, `\n` a newline,
`\r` a carriage return, and `\\` a literal backslash. Any other backslash
sequence is kept as-is, so a separator like `\d` really joins with the two
characters `\d`.

</details>

<details>
<summary>How do I turn a list of IDs into a SQL IN (...) clause?</summary>

Set **Prefix** and **Suffix** both to `'` and keep the default `, ` separator:

```
alice
bob
```

joins to `'alice', 'bob'` — paste it straight into `IN (…)`. The prefix and
suffix wrap every line *before* joining, so quotes never end up around the
separator.

</details>

<details>
<summary>What counts as a blank line for "Remove blank lines"?</summary>

Empty lines **and** whitespace-only lines (spaces or tabs) are dropped, even
when "Trim each line" is off. Dropped lines get no prefix, suffix or
separator, so you never see `'', ''` artifacts from stray empty rows at the
end of a paste.

</details>

<details>
<summary>What happens if I leave the separator blank?</summary>

The lines are concatenated with nothing between them — useful for stitching a
wrapped Base64 string or hex dump back into one unbroken token. (The default,
when the field is untouched, is `", "`.)

</details>
