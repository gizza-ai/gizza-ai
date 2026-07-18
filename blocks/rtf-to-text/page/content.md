## Convert RTF source to readable plain text

RTF files are text documents filled with formatting commands such as `\b`, `\fs24`, font tables, color tables, and nested groups. Paste the raw `{\rtf ...}` source here to strip those controls and keep the visible text.

The converter decodes common RTF escapes along the way: `\'hh` Windows-1252 bytes, `\uN` Unicode escapes, smart quote controls, bullets, em dashes, paragraph breaks, tabs, table cells, and escaped literal braces. Non-text destinations such as font/color tables, metadata, pictures, stylesheets, and ignorable `\*` groups are skipped.

### Worked example

Input:

```rtf
{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}\pard The quick \b brown\b0  fox.\par Caf\'e9 costs 5\'80.\par}
```

With **Line breaks** set to **Preserve paragraphs**, the output is:

```text
The quick brown fox.
Café costs 5€.
```

Choose **Collapse to one line** when you want search-index or prompt-ready text with every run of whitespace flattened to a single space.

<details>
<summary>Can I upload a .rtf file?</summary>

This page accepts pasted RTF source. Open the `.rtf` file in a plain-text editor, copy the markup that starts with `{\rtf`, and paste it into the input box.

</details>

<details>
<summary>Does it preserve formatting such as bold, fonts, or colors?</summary>

No. The goal is plain text. Formatting controls, font tables, color tables, styles, metadata, and embedded pictures are removed while the readable characters are kept.

</details>

<details>
<summary>Which character encodings are decoded?</summary>

The parser decodes RTF `\uN` Unicode escapes and Windows-1252 `\'hh` hex escapes, including smart quotes, en/em dashes, the euro sign, accented Latin text, emoji, and non-Latin scripts when represented by Unicode escapes.

</details>

<details>
<summary>What happens with malformed RTF?</summary>

The tool returns an error if the input does not start like an RTF document. For truncated groups or partial escapes inside an otherwise recognizable RTF document, it extracts the text it can read without panicking.

</details>

### Limits

- This is a pragmatic text extractor, not a full RTF layout engine.
- Embedded images and OLE objects are ignored.
- Pasted input must be raw RTF markup beginning with `{\rtf`; rich clipboard formatting is not enough.
