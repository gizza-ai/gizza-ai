## About this tool

**RTF to Markdown Converter** turns pasted Rich Text Format source into clean Markdown. It is useful when you have legacy `.rtf` text from an editor, export, email archive, or clipboard pipeline and need something readable for docs, READMEs, notes, or AI prompts.

The converter preserves the structure Markdown can represent: bold, italic, strikethrough, headings from outline/style metadata, bullet and numbered lists, hyperlinks, Unicode escapes, superscript/subscript, and simple tables. Underlines can be kept as inline `<u>` HTML because Markdown has no native underline syntax, or dropped while keeping the text.

### Worked example

Input RTF:

```
{\rtf1\ansi{\pard\outlinelevel0 Project notes\par}This is \b bold\b0  and \i italic\i0  text.\par}
```

Markdown output:

```
# Project notes

This is **bold** and *italic* text.
```

### Options

- **Headings** — detect headings from `\outlinelevelN` and stylesheet names like `heading 1`, or turn detection off and render every paragraph as body text.
- **Tables** — output GitHub-style Markdown pipe tables, or tab-separated rows when you need a plain-text table.
- **Underline** — keep underlines as `<u>text</u>` HTML, or ignore underline styling.
- **Convert hyperlinks** — when enabled, RTF `HYPERLINK` fields become `[text](url)` links; when disabled, only the visible text remains.
- **Escape literal Markdown punctuation** — keep stray `*`, `_`, `[`, and `|` characters from being interpreted as Markdown.

### Limits and edge cases

- Paste raw RTF source that begins with `{\rtf`; this tool does not read binary Word `.doc` or `.docx` files.
- RTF supports colors, fonts, margins, revisions, images, and embedded objects. Markdown does not, so those visual/document metadata destinations are intentionally skipped.
- Complex merged or nested tables are simplified to readable rows; choose tab-separated table output if pipe tables would be misleading.
- The parser is designed for common RTF from editors and exports, not for damaged or encrypted documents.

## FAQ

<details>
<summary>Can this convert a Word document?</summary>

Only if you have the document's RTF source. Save or export the Word content as Rich Text Format, then paste the text that starts with `{\rtf`. Binary `.doc` and zipped `.docx` files are different formats and are outside this tool's model.

</details>

<details>
<summary>What formatting is preserved?</summary>

The converter keeps formatting that maps cleanly to Markdown: headings, paragraphs, bold, italic, strikethrough, lists, simple tables, links, superscript/subscript, Unicode escapes, and optional underline HTML. Fonts, colors, margins, pictures, comments, and revision metadata are skipped because Markdown has no direct equivalent.

</details>

<details>
<summary>Why do underlines become HTML?</summary>

Markdown has no standard underline marker. Keeping underline as `<u>underlined text</u>` is the most portable way to preserve the signal in Markdown renderers that allow inline HTML. If you want pure Markdown text, set **Underline** to **Drop underline markup**.

</details>

<details>
<summary>Is the RTF uploaded anywhere?</summary>

No. The page runs a WebAssembly converter in your browser. The RTF text is processed locally, and you can copy or download the Markdown result without sending the content to a server.

</details>
