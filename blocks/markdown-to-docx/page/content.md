## Convert Markdown into an editable Word document

Paste Markdown and download a genuine Office Open XML `.docx` file — the same ZIP-based format Word,
Google Docs, Pages and LibreOffice Writer open natively. Headings become Word heading styles, bullets
and numbered items become native lists, block quotes get quote styling, fenced code blocks use a
monospace shaded paragraph, and inline **bold**, *italic*, `` `code` `` and `~~strikethrough~~` are
preserved as separate Word runs.

Choose **Letter** or **A4**, select a body font, set the base font size, and decide whether a thematic
break (`---`) should draw a horizontal rule or start a new page. Everything runs in your browser with
WebAssembly, so the Markdown never needs to be uploaded.

### Worked example

Input:

```markdown
# Project brief

This is **ready** for review.

- Scope
- Timeline

> Keep a copy for stakeholders.
```

Output: a `document.docx` with a Heading 1 paragraph, formatted bold text, two list items and a styled
quote paragraph. Add a **Document title** to prepend a title paragraph and store the title in the DOCX
metadata, or enable **Treat --- as a page break** for report-style section breaks.

## Limits and edge cases

This is an offline structural converter, not a full browser or Word clone. Remote images are rendered
as their alt text because the block does not fetch external files. LaTeX math, Mermaid diagrams,
syntax-colored code highlighting and generated tables of contents are intentionally out of scope. Very
large documents are limited by browser memory and the chat surface's 24 MB output cap.

## FAQ

<details>
<summary>Is the download a real .docx file?</summary>

Yes. The tool writes an Office Open XML package with `[Content_Types].xml`, relationships, document,
styles, numbering and document property parts. It is not Markdown renamed with a `.docx` extension.

</details>

<details>
<summary>Which Markdown features are preserved?</summary>

Headings, paragraphs, bold, italic, inline code, strikethrough, bullets, numbered lists, nested list
indentation, block quotes, fenced code blocks and thematic breaks are converted into editable Word
structure. Links keep their visible label text; remote image syntax keeps the alt text.

</details>

<details>
<summary>Can I control the Word page setup?</summary>

Yes. Choose Letter or A4 page size, Calibri, Aptos, Times New Roman or Arial as the body font, and an
8–24 point base font size. Headings scale relative to that base size.

</details>

<details>
<summary>Does my Markdown leave the browser?</summary>

No. The page version runs the converter in WebAssembly and creates a local `data:` URL for the download.
No upload service is contacted.

</details>
