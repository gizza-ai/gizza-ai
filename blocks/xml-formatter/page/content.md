## About this tool

**XML formatter** cleans up XML two ways:

- **Pretty-print** — re-indent the XML so its structure is easy to read, with a
  configurable number of spaces per level.
- **Minify** — strip insignificant whitespace into a single compact line, smaller
  for transport or storage.

It also **checks well-formedness**: if the XML has a mismatched tag or a syntax
error, you get a clear message with the byte position instead of broken output.

### Privacy

Everything runs **in your browser** via WebAssembly — your XML is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Common uses

- Make machine-generated or one-line XML readable for debugging.
- Shrink XML before sending it over the wire.
- Quickly confirm an XML snippet is well-formed.

## FAQ

<details>
<summary>Does it validate my XML against an XSD schema or DTD?</summary>

No — it checks **well-formedness** only: balanced tags, proper nesting, legal
syntax. That's enough to catch a missing closing tag or an unescaped `&`, but it
doesn't know anything about your schema, so structurally "wrong" but well-formed
documents format without complaint.

</details>

<details>
<summary>What does "XML is not well-formed at byte N" point at?</summary>

`N` is the **byte offset** where the parser gave up — count bytes from the start
of your input (multi-byte UTF-8 characters count more than one). The usual
culprits are a mismatched or unclosed tag, an attribute missing its quotes, or a
bare `&` that should be `&amp;`.

</details>

<details>
<summary>Are comments, CDATA sections, and the XML declaration kept?</summary>

Yes. The formatter re-emits every parsed event — comments, `<![CDATA[…]]>`
sections, processing instructions, and the `<?xml …?>` declaration all pass
through in both pretty and minify modes; only the whitespace *between* elements
changes.

</details>

<details>
<summary>How much can I indent, and does minify use the indent value?</summary>

Pretty mode indents 0–16 spaces per level (default 2); values outside that range
are clamped. Minify ignores the indent entirely — it collapses insignificant
whitespace so the document comes out as a single compact line.

</details>
