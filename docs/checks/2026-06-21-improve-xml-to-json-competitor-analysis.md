# xml-to-json — competitor analysis (2026-06-21)

Tool: `blocks/xml-to-json` — convert an XML document into an equivalent JSON
structure, preserving attributes and nesting. Pure-Rust (`quick-xml` +
`serde_json` with `preserve_order`), runs on all backends (chat / CLI / page),
fully local, deterministic, no AI model.

## Surfaces verified

- **Chat block**: `wafer build` OK — block instantiates (376.5 KiB), schema
  drift-guard test passes.
- **CLI**: `gizza tool xml-to-json` — basic conversion, `attributes=false`,
  `coerce_types=true`, and a malformed-XML error path all produce correct output.
- **Page**: 3 Playwright tests pass — attribute/nesting/array preservation,
  attribute-drop + type coercion toggles, and query-param prefill (`@`-prefix
  override).
- **Unit**: 14 core tests + 1 schema-drift test green.

## Competitors surveyed (top 5)

1. **FreeFormatter** (freeformatter.com/xml-to-json-converter.html)
2. **JSONFormatter** (jsonformatter.org/xml-to-json)
3. **Code Beautify** (codebeautify.org/xmltojson)
4. **JSONLint** (jsonlint.com/xml-to-json)
5. **Site24x7 / ToolsLab / GeeksforGeeks** (generic browser converters)

## Capability matrix (✓ = supported by gizza, after this build)

| Capability                                            | Competitors | gizza xml-to-json |
|-------------------------------------------------------|-------------|-------------------|
| Element → object keyed by tag                         | ✓           | ✓                 |
| Configurable attribute prefix (default `@`)           | ✓ (`@`)     | ✓ (`attribute_prefix`, default `@`) |
| Toggle attributes on/off                              | partial     | ✓ (`attributes`)  |
| Configurable mixed-content text key (default `#text`) | ✓           | ✓ (`text_key`)    |
| Repeated siblings → JSON array, in document order     | ✓           | ✓                 |
| Type coercion (number/bool/null)                      | ✓ (opt-in)  | ✓ (`coerce_types`, opt-in; leading zeros kept as strings) |
| CDATA + entity decoding                               | ✓           | ✓                 |
| Namespace prefix stripped to local name               | partial     | ✓                 |
| Comments / PIs / declaration ignored                  | ✓           | ✓                 |
| Pretty-printed JSON output                             | ✓           | ✓                 |
| Runs fully in-browser / private (no upload)           | some        | ✓ (wasm, all surfaces local) |
| LLM/chat surface + CLI                                  | ✗           | ✓ (unique to gizza) |

## In-model gaps closed in this build

All competitor features that fit gizza's pure-text-in/text-out, single-input
page model are implemented: configurable attribute prefix, attribute toggle,
configurable text key, array collapsing in document order, opt-in type coercion
(with leading-zero protection), CDATA/entity decoding, and pretty output. The
schema descriptions were written to guide an LLM through each option in chat.

## Out-of-model features (intentionally not built)

- **URL / file upload input** — the gizza tool page is a single text field;
  there is no file-input affordance for pure tools (consistent with the rest of
  the text-conversion catalog). CLI accepts inline `xml=…`.
- **Code-snippet generation** (PHP/Python/JS/Java/.NET/curl) — out of scope for
  a converter; belongs to a different tool class.
- **Configurable indentation / bracket style / compact output** — gizza emits a
  single canonical 2-space pretty form; a JSON beautifier/minifier already
  exists (`json-beautify`) for reformatting.
- **"Produce JSON even if XML is malformed"** — deliberately rejected; gizza
  returns a precise parse error with byte position rather than silently
  guessing, which is safer and more honest.

## Distinctiveness vs existing gizza blocks

Not a duplicate: `xml-to-csv` flattens repeated records into a CSV table,
`csv-to-xml` goes the other direction, and `xml-formatter` only pretty-prints
XML. None produce a JSON structure. `json-yaml-convert` / `csv-json-convert`
handle other source formats, not XML. This fills the XML→JSON conversion slot.
