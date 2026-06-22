## About this tool

XML to JSON converts an XML document into an equivalent JSON structure, keeping
the shape of your data intact: attributes, nested elements, and repeated tags all
map predictably to JSON. Paste your XML and get clean, pretty-printed JSON back —
everything runs locally in your browser, so nothing is uploaded.

## How the conversion works

- **Elements** become JSON objects keyed by their tag name. The document's single
  root element becomes the only top-level key.
- **Attributes** become object members prefixed with `@` by default (so
  `id="1"` becomes `"@id": "1"`). Change the prefix, or turn attributes off
  entirely.
- **Repeated sibling tags** collapse into a JSON array, in document order — so
  three `<book>` elements become a `"book": [ … ]` array.
- **Text content** of a simple element becomes a plain string. When an element
  has both text and attributes or children, its text is stored under a
  configurable key (`#text` by default).
- **Entities and CDATA** are decoded, comments and processing instructions are
  ignored, and namespace prefixes are reduced to their local names.

## Type coercion

By default every value stays a string, which is the safest round-trip. Enable
"Coerce numbers, booleans, and null" to turn text like `42`, `1.5`, `true`, and
`null` into the matching JSON scalar. Leading-zero strings such as `007` are kept
as strings so identifiers are never mangled.

## Private and offline

The conversion runs entirely in your browser via WebAssembly. Your XML never
leaves your device — there is no upload, no account, and no tracking.
