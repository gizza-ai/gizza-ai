## Markdown to Confluence, right in your browser

Paste a Markdown document and get clean Confluence markup you can drop straight
into a page. The conversion runs entirely on your device with WebAssembly — your
text is never uploaded, there is no account, and there are no limits. Pick
**storage format** for the XHTML the Cloud REST API consumes, or **wiki markup**
for the Data Center / Server "Insert markup" dialog.

## Which output format do I want?

- **Storage format** is the canonical XHTML-based markup a Confluence Cloud page
  is stored as. It carries structured macros — so fenced code becomes a real
  **code macro** (with its language) and a `Note:` blockquote becomes a real
  **panel** — and it's what the REST API (`PUT /content`) expects. Use it when
  you're pushing pages programmatically or pasting into the storage-format editor.
- **Wiki markup** is the older text dialect used by the Data Center / Server
  editor's "Insert markup" flow. It's more compact (`h2.`, `*bold*`, `{code}`,
  `{note}`) but the Cloud editor won't paste it directly.

## What gets converted

- **Headings** — ATX `#` … `######` map to `h1`–`h6`. Use the **heading offset**
  option to demote every heading by 1–5 levels when you're pasting under an
  existing page title; anything past `h6` is capped there.
- **Inline formatting** — `**bold**`, `*italic*`, `~~strikethrough~~`, and
  `` `inline code` `` all convert, in both formats.
- **Lists** — bullet, numbered, and nested lists become `<ul>`/`<ol>` in storage
  format or `*` / `#` markers (repeated for nesting) in wiki markup.
- **Code** — fenced blocks become a **code macro** in storage format (with the
  language you tagged the fence, inside a `CDATA` block so nothing is escaped) or
  `{code:language}` in wiki markup.
- **Tables** — GitHub pipe tables become a `<table>` with `<th>` header cells, or
  `||header||` / `|cell|` rows in wiki markup.
- **Panels** — a blockquote whose first line starts `Note:`, `Warning:`, `Info:`,
  or `Tip:` becomes the matching Confluence panel macro (`info` / `note` /
  `warning` / `tip`), with the prefix stripped. Turn the option off to keep every
  blockquote a literal quote.
- **Links, images, and rules** — `[text](url)`, `![alt](src)`, and `---`
  thematic breaks all convert to their Confluence equivalents.

## Safe by construction

Prose is escaped for the target format automatically — the XHTML special
characters (`&`, `<`, `>`) in storage format and the structural characters
(`{`, `}`, `[`, `]`, `|`) in wiki markup — so a `1 < 2` comparison or a
`{placeholder}` won't break the page. Code blocks are emitted literally, so what
you paste is exactly what runs.

## Private by design

The conversion happens locally in your browser with WebAssembly. Nothing is sent
to a server, so it works offline and your documents stay yours.

## FAQ

<details>
<summary>What is the difference between storage format and wiki markup?</summary>

Storage format is the XHTML-based markup a Confluence Cloud page is actually
stored as, and it's what the REST API consumes — it carries structured macros
like the code and panel macros. Wiki markup is the older, more compact text
dialect used by the Data Center / Server editor's "Insert markup" dialog. Pick
storage format for Cloud and the API; pick wiki markup for the legacy insert flow.

</details>

<details>
<summary>How do I get an info, note, warning, or tip panel?</summary>

Write a blockquote whose first line starts with `Note:`, `Warning:`, `Info:`, or
`Tip:` (case-insensitive) — for example `> Warning: back up first.` With the
panel option on (the default), it becomes the matching Confluence panel macro and
the prefix is removed. Turn the option off if you'd rather keep every blockquote
as a plain quote.

</details>

<details>
<summary>Can I paste the wiki-markup output straight into Confluence Cloud?</summary>

Not directly — the Cloud editor doesn't accept wiki markup on paste. Use the
storage format output for Cloud (via the storage editor or the REST API), and use
wiki markup for the Data Center / Server "Insert markup" dialog, which still
supports it.

</details>

<details>
<summary>What does the heading offset do?</summary>

It demotes every heading by 1–5 levels: with offset 1, `#` becomes `h2` instead
of `h1`, `##` becomes `h3`, and so on. That's the right knob when you're pasting
the converted content under an existing page title. Levels that would fall past
`h6` stay at `h6`.

</details>

<details>
<summary>Does anything get uploaded?</summary>

No — the whole conversion runs in your browser with WebAssembly. Your Markdown
never leaves your device, there's no account, and it works offline once the page
has loaded.

</details>
