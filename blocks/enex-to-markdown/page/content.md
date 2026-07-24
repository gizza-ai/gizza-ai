## About this tool

This converter turns an Evernote ENEX export into a single Markdown or plain-text document. ENEX files are XML: each `<note>` contains an ENML/HTML body, a title, timestamps, tags, optional source URL metadata, and optional base64 resources. The tool parses those notes locally, converts the ENML body to Markdown, and joins multiple notes with horizontal rules.

A minimal example:

```xml
<en-export>
  <note>
    <title>Migration note</title>
    <content><![CDATA[<en-note><h1>Hello</h1><p>Some <b>bold</b> text.</p></en-note>]]></content>
    <created>20230101T090000Z</created>
    <tag>import</tag>
  </note>
</en-export>
```

With **Markdown** and **YAML frontmatter**, the output includes the title, ISO-formatted date, tags, and converted body:

```markdown
---
title: Migration note
created: 2023-01-01T09:00:00Z
tags: [import]
---

# Hello

Some **bold** text.
```

Use **Metadata style** to choose YAML frontmatter, inline metadata with `#tags`, or title-only output. Turn **List attachments** on to add an attachment section for each note; the tool reports filename, MIME type, and decoded size, but it does not emit the binary attachment files.

### Limits

This is a text-output converter, not a full note-migration app. It concatenates notes into one document rather than writing one file per note, reports attachments instead of extracting them, and does not implement app-specific Obsidian/LogSeq/Tana presets. ENML checkboxes and some Evernote-only styling may be simplified by the HTML-to-Markdown pass. Very large exports can produce large output, so use smaller notebook-level ENEX exports when practical.

## FAQ

<details>
<summary>Does this upload my Evernote export?</summary>

No. The parser and Markdown conversion run as WebAssembly in your browser tab. The ENEX XML you paste stays on your device; there is no server upload.

</details>

<details>
<summary>What happens to images and PDF attachments?</summary>

Attachments are listed per note with their filename, MIME type, and decoded size. The binary payloads are not written out because this tool has one text output surface. If you need a full folder tree of resources, use a dedicated migration tool outside the browser.

</details>

<details>
<summary>Can I import the Markdown into Obsidian or another notes app?</summary>

Yes, the Markdown is generic and includes titles, body content, tags, dates, and source URLs. App-specific link syntaxes and vault folder layouts are not generated; copy the output into your notes app and adjust structure as needed.

</details>

<details>
<summary>Why are multiple notes joined with horizontal rules?</summary>

An ENEX export can contain many notes, while this page returns one text result. Horizontal rules keep each note visually separated in the combined Markdown. For large migrations, export one notebook at a time or split the output after conversion.

</details>

<details>
<summary>What if the ENEX XML is malformed?</summary>

The converter stops with an error that points to malformed ENEX/XML input. Re-export from Evernote if possible, or check that the file was copied completely including its closing `</en-export>` tag.

</details>
