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

## FAQ

<details>
<summary>Why is my element sometimes an object and sometimes an array?</summary>

Repeated sibling tags collapse into an array, but a tag that appears only once
stays a single value — `<catalog><book>A</book><book>B</book></catalog>` gives
`"book": ["A", "B"]`, while one `<book>` gives `"book": "A"`. If your code
expects an array either way, normalize after conversion (wrap non-arrays); the
mapping itself always mirrors what's actually in the document.

</details>

<details>
<summary>What does an empty element like &lt;a/&gt; turn into?</summary>

`null`. An element with no attributes, no children, and no text maps to JSON
`null` rather than an empty string or empty object, so
`<root><a/></root>` becomes `{"root": {"a": null}}`.

</details>

<details>
<summary>Why did "42" come out as a string instead of a number?</summary>

Type coercion is off by default because keeping everything as strings is the
safest round-trip. Enable the coercion option to convert integer, float,
`true`/`false` and `null` text into real JSON scalars. Even with coercion on,
leading-zero values like `007` deliberately stay strings so IDs, ZIP codes,
and phone numbers aren't mangled.

</details>

<details>
<summary>Why do I get a "multiple root elements" error?</summary>

Well-formed XML must have exactly one root element, and the converter enforces
that — a fragment like `<a/><b/>` is rejected. Wrap sibling fragments in a
single enclosing element and convert that. Other malformed input (mismatched
tags, unclosed elements) is reported with the byte position where parsing
failed.

</details>
