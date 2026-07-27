# json-to-xml — competitor analysis (2026-07-24)

Tool: render JSON as well-formed XML with configurable root and item element names.
Pure text transform, so it fits the gizza model with chat, CLI, and standalone browser page.

## Competitors skimmed (top reachable tools)

1. **FreeFormatter JSON to XML Converter** — paste JSON, choose output with a default root, and convert to XML. The page emphasizes formatting and validation-style error messages.
2. **ConvertJSON JSON to XML** — paste/upload-style JSON input, convert arrays and nested objects, and copy the result. Includes examples and simple conversion controls.
3. **Code Beautify JSON to XML Converter** — paste JSON and download/copy XML, with tree-ish examples and pretty output. Adjacent tools offer minify/beautify behavior.

## Table-stakes → decision

Core conversion:
- JSON object/array/scalar input — **in** (`json`, required multiline text).
- Configurable root element — **in** (`root_element`, default `root`).
- Configurable array item element — **in** (`array_item_element`, default `item`).
- Pretty output — **in** (`format=pretty`, default; `indent` spaces 0-8).
- Compact output — **in** (`format=compact`).
- Optional XML declaration — **in** (`xml_declaration`).
- XML escaping for text and attributes — **in**.
- Invalid XML name handling — **in** (sanitize keys rather than emit invalid XML).

Mapping conventions:
- Attribute convention — **in** (`attribute_prefix`, default `@`, set empty to disable).
- Text-node convention — **in** (`text_key`, default `#text`).
- Null values — **in** as empty self-closing tags.

UX/page expectations:
- Large textarea with example JSON placeholder — **in**.
- Select control for pretty/compact — **in**.
- Checkbox for XML declaration — **in**.
- Preset chips for common examples — **in**.
- Exact output and deep-link page tests — **in**.

Out of model / not built:
- XML Schema (XSD) generation or validation — requires schema inference and validation semantics beyond a converter.
- Namespace-aware mapping UI — JSON has no native namespace model; users can still include colon names.
- File upload/download-specific workflow — text paste/download covers the public toolkit model; download is generic for text pages.

No competitor copy, branding, or trademarks were reused; this is a paraphrased feature scan and fit decision log.
