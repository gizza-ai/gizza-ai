## About this tool

**JSON Escape / Unescape** makes a string safe to drop inside a JSON document —
or reverses the process.

- **Escape** turns raw text into a JSON-safe string body: double quotes become
  `\"`, backslashes `\\`, newlines `\n`, tabs `\t`, and other control characters
  `\uXXXX`. Tick **Wrap in quotes** to also add the surrounding `"…"`.
- **Unescape** turns an escaped JSON string back into raw text. It accepts input
  with or without the surrounding quotes.

The escaping is done with a spec-correct JSON codec, so it matches exactly what a
real JSON parser expects. Everything runs **locally in your browser** via
WebAssembly — nothing is uploaded.

### Handy for

- Pasting a multi-line snippet into a JSON config or API payload.
- Reading an escaped value out of a log or JSON blob.
- Embedding text in code that builds JSON by hand.
