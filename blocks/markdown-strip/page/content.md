## About this tool

**Markdown Stripper** removes Markdown formatting from any text and returns clean,
readable plain text. It parses your input as CommonMark (plus GitHub-flavored
tables, strikethrough, task lists, and footnotes) and keeps the words while
dropping the syntax — so you get exactly the visible content with none of the
markup. Everything runs locally in your browser: nothing is uploaded.

It removes heading `#` markers, **bold**/*italic*/~~strikethrough~~ emphasis,
blockquote `>` markers, and horizontal rules. Code stays readable — the fences and
backticks go, but the code text inside is kept verbatim. Tables collapse to bare
cell text (cells joined by spaces, one row per line). You control how links and
images are handled, and whether list markers are preserved.

### Worked example

Input Markdown:

```
# Weekly update

Shipped **login** and fixed the [signup bug](https://example.com/issues/42).

- Deploy on Friday
- `cargo test` is green
```

Plain-text output (defaults — keep link text, remove list markers, collapse blank
lines):

```
Weekly update
Shipped login and fixed the signup bug.
Deploy on Friday
cargo test is green
```

### Options

- **Links** — keep the visible link text (default), keep the URL, or keep both as
  `text (url)`.
- **Images** — keep the image's alt text (default), or remove images entirely.
- **Keep list markers** — off by default (one item per line); turn it on to keep
  `-` bullets and `1.` numbering.
- **Collapse blank lines** — on by default (blocks separated by a single newline);
  turn it off to keep a blank line between blocks.

### Limits

- The input is treated as Markdown, so a stray `#`, `*`, or `_` in prose may be
  interpreted as formatting. This is the same behavior any Markdown renderer has.
- LaTeX math is not converted to Unicode symbols — the text is passed through.
- Everything happens in-browser on a single paste; there is no file upload or
  server-side batch. Use **Copy** or **Download** for the result.

## FAQ

<details>
<summary>Does it keep the text inside code blocks?</summary>

Yes. Only the fences (` ``` `) and inline backticks are removed — the code text
itself is preserved exactly, including its line breaks. This makes it safe for
cleaning up documentation or AI chat output without losing snippets.

</details>

<details>
<summary>What happens to links and images?</summary>

Links default to keeping the visible label and dropping the URL (`[docs](…)` →
`docs`). Switch **Links** to keep the URL instead, or to keep both as
`docs (https://…)`. Images default to their alt text; set **Images** to *Remove
images* to drop them completely.

</details>

<details>
<summary>How are tables handled?</summary>

A Markdown table becomes bare cell text: the pipes and the `---` separator row are
removed, each row is one line, and the cells in a row are joined by spaces. It is a
readable flattening, not a CSV export — use a dedicated Markdown-table-to-CSV tool
if you need columns.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely in your browser. Your
Markdown never leaves your device, so it is safe to paste private or unpublished
content.

</details>
