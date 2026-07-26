## About this tool

Jira's classic wiki renderer uses a syntax that looks like Markdown but is not
Markdown: headings are `h2. Title`, bold is `*bold*`, inline code is `{{code}}`,
links are `[text|url]`, and tables use `||header||` rows. This converter turns
standard Markdown into that Jira wiki markup, and can convert the common Jira
subset back into Markdown when you are moving issue descriptions, comments, or
runbooks between tools.

Use **Direction** to choose the conversion path:

- **Markdown → Jira** converts headings, bold/italic/strike, inline code, fenced
  code blocks, links, images, ordered and unordered lists, nested list markers,
  pipe tables, blockquotes, horizontal rules, and Note/Info/Warning/Tip panel
  blockquotes.
- **Jira → Markdown** converts the same common wiki markup subset back to
  portable Markdown.

Set **Heading offset** when you are pasting Markdown under an existing Jira page
title. For example, offset `1` turns `# Title` into `h2. Title` instead of `h1.
Title`. Leave **panel blockquotes** enabled to map blockquotes such as `> Warning:
Check this` to Jira's `{warning}` macro.

Everything runs locally in your browser. No text is uploaded, logged, or cached by
a server.

### Worked example

Markdown input:

````markdown
# Release notes

**Fixed:** login timeout

- Faster dashboard
- Better export

```js
console.log('done')
```
````

Jira wiki markup output:

```text
h1. Release notes

*Fixed:* login timeout

* Faster dashboard
* Better export

{code:js}
console.log('done')
{code}
```

## Limits and edge cases

This is a wiki-markup converter, not an Atlassian Document Format (ADF) JSON
exporter. It intentionally keeps to syntax that has a clear Markdown equivalent.
Jira-only macros such as custom `{panel:title=...}`, color spans, underline,
superscript, subtasks, mentions, attachments, footnotes, and task checkboxes are
left as plain text or documented rather than guessed. Jira and Markdown also have
no shared model for table alignment, colspan/rowspan, or cell colors, so those are
not preserved.

## FAQ

<details>
<summary>Can I paste the output directly into Jira?</summary>

Yes, for Jira projects that still accept classic wiki markup in issue comments,
descriptions, or older editor fields. Some Jira Cloud surfaces use Atlassian
Document Format internally; this tool does not generate ADF JSON.

</details>

<details>
<summary>Does it convert Jira wiki markup back to Markdown?</summary>

Yes. Choose **Jira → Markdown** to convert common Jira headings, lists, code
blocks, links, images, quotes, tables, and inline styles back into Markdown. Jira
macros without a Markdown equivalent are preserved as text rather than silently
dropped.

</details>

<details>
<summary>How are notes and warning panels handled?</summary>

With panel blockquotes enabled, Markdown blockquotes that start with `Note:`,
`Info:`, `Warning:`, `Warn:`, or `Tip:` become Jira `{note}`, `{info}`,
`{warning}`, or `{tip}` blocks. On the reverse path those macros become Markdown
blockquotes with the same label.

</details>

<details>
<summary>Is my issue text uploaded anywhere?</summary>

No. The converter runs entirely in your browser via WebAssembly and the CLI uses
the local `gizza` binary. Your Markdown or Jira text stays on your machine.

</details>

<details>
<summary>Why is the output not pixel-identical after a round trip?</summary>

Markdown and Jira wiki markup do not represent every feature the same way. The
converter prioritizes preserving structure and readable content for common issue
and documentation markup, not byte-for-byte source formatting.

</details>
