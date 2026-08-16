## Pull every table out of a Markdown document

Paste a whole README, changelog or spec — not just a lone table. Every
GitHub-flavored pipe table in it is found, in document order, and exported as
**CSV**, **JSON**, **JSON Lines**, or a **list** of what the document contains.
Everything runs locally in your browser; the text is never uploaded.

A table is recognised the way GitHub recognises one: a header row containing
pipes, followed directly by a separator row (`| --- |`, `|:---|`, `|---:|`,
`|:--:|`) with the **same number of cells**. Outer pipes are optional, and pipe
lines inside ` ``` ` or `~~~` code fences are skipped — so the SQL snippet in
your README is never mistaken for data.

### Worked example

Input:

```markdown
# Release 2.1

## Downloads

| file | size |
| --- | ---: |
| app-linux.tar.gz | 12 MB |
| app-macos.zip | 14 MB |

## Plans

| plan | seats |
|:---|---:|
| Free | 3 |
| Team | 25 |
```

With the defaults (CSV, all tables), the output is:

```csv
# Table 0: Downloads
file,size
app-linux.tar.gz,12 MB
app-macos.zip,14 MB

# Table 1: Plans
plan,seats
Free,3
Team,25
```

Switch **Which tables** to `1` and **Output format** to `json` and you get just
that table's rows:

```json
[
  {
    "plan": "Free",
    "seats": "3"
  },
  {
    "plan": "Team",
    "seats": "25"
  }
]
```

### Options

- **Output format** — `csv` (default), `json`, `jsonl`, or `list`.
  - `csv` writes one delimited block per table, separated by a blank line.
  - `json` gives an array of row objects for a single table, or an array of
    `{index, heading, line, columns, rows}` envelopes when several are exported.
  - `jsonl` gives one JSON value per data row; with several tables each line is
    wrapped as `{"table": n, "row": …}`.
  - `list` gives an inventory only — index, nearest preceding heading, source
    line, column names, column alignments and row count — with no cell data.
- **Which tables** — `all` (default), a single 0-based index like `2`, or a
  comma-separated list/range like `0,2-3`. Run `list` first if you're not sure.
- **First row is a header** — on by default. CSV keeps it as the first line and
  JSON/JSON Lines key each row object by it. Off, the header row is dropped and
  JSON emits plain arrays of values.
- **CSV delimiter / quoting / line endings** — a single character or
  `comma`/`tab`/`semicolon`/`pipe`/`space`; quote only when needed (default) or
  every field; LF (default) or CRLF for Windows and Excel.
- **Trim padding inside cells** — on by default, so the spaces authors use to
  align columns don't end up in your data.
- **Render cell Markdown as plain text** — off by default (cells come out
  exactly as written). On, `**bold**` becomes `bold`, `` `code` `` becomes
  `code`, `[text](url)` becomes `text`, `<br>` becomes a space, and `\|`-style
  escapes are resolved.
- **JSON indent** — `0`–`8` spaces; `0` minifies to a single line.
- **Label each CSV block** — on by default; adds a `# Table n: heading` comment
  above each block when more than one table is exported.

### Limits and edge cases

- Documents up to **1,000,000 bytes** (about 1 MB of Markdown).
- Rows with **fewer** cells than the header are padded with empty values; cells
  **past** the header count are dropped — exactly what a Markdown renderer shows.
- A table with a header and separator row but no data rows still exports (you
  get the header line, or an empty array).
- `\|` inside a cell is kept as a literal pipe, and `\\` as a literal backslash.
- A pipe line with no separator row under it is prose, not a table, and is
  ignored. If nothing in the document qualifies you get an explicit error saying
  what a table needs, rather than empty output.

## FAQ

<details>
<summary>My document has several tables — how do I get just one?</summary>

Set **Which tables** to its 0-based index: `0` is the first table in the
document, `1` the second, and so on. You can also pass a list or range such as
`0,2-3`. Pick `list` as the output format first to see every table's index,
heading and column names, so you know which number you want. An index past the
last table returns an error naming the valid range instead of silently giving
you the wrong data.

</details>

<details>
<summary>Why is the table in my code block being ignored?</summary>

That's deliberate. Documentation is full of fenced snippets that contain pipes —
shell pipelines, SQL, ASCII art — and treating them as data produces garbage.
Anything between ` ``` ` or `~~~` fences is skipped. If you actually want that
table extracted, remove the fence around it, or paste just the table on its own.

</details>

<details>
<summary>What happens to bold text, code spans and links inside cells?</summary>

By default nothing — cells come out byte-for-byte as written, so
`[Guide](https://example.com)` stays as that whole string. That's the lossless
choice. Turn on **Render cell Markdown as plain text** to get reading text
instead: bold and italic markers are removed, code spans are unwrapped, a link
becomes just its label, `<br>` becomes a space, and runs of whitespace collapse.
Underscores inside words (`snake_case`) are left alone.

</details>

<details>
<summary>What's the difference between JSON and JSON Lines here?</summary>

JSON gives you one document: an array of row objects when you export a single
table, or an array of table envelopes (index, heading, source line, columns,
rows) when you export several. JSON Lines gives you one compact JSON value per
line, which is what log pipelines and tools like `jq -c` prefer; when more than
one table is exported each line is wrapped as `{"table": n, "row": …}` so you
can tell the rows apart.

</details>

<details>
<summary>Will the CSV open cleanly in Excel or Google Sheets?</summary>

Yes. Fields are quoted and escaped per RFC 4180, so commas, quotes and newlines
inside a cell survive. For Excel on Windows, switch **Line endings** to CRLF.
For a TSV, set the delimiter to `tab`. If you're exporting several tables at
once, either turn off the `# Table n` label lines or split the blocks apart
first — a spreadsheet import expects one table per file.

</details>
