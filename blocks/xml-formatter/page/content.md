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
