# xsd-to-json-schema — competitor analysis (2026-09-23)

Scan run **before** implementing, per the create-next-tool recipe. All findings are paraphrased
observations of publicly documented behaviour; no competitor copy, branding, or trademarks are
reproduced or reused. Out-of-model items are listed, not built.

## Search

One search for the tool's function: *"XSD to JSON Schema converter online tool"*. The result set
splits into two groups — browser-side converters (the real comparables for a gizza tool) and
desktop/IDE XML suites (relevant only for feature vocabulary, since they are licensed products
with a server/desktop model gizza does not share).

## Competitors skimmed

### 1. jsonformatterspro.com — XSD to JSON Schema (browser, client-side)

- **Output dialect:** Draft-07 only, justified as the widest-validator-compatibility choice
  (Python/Java/Node validators).
- **Mappings documented:** `complexType` → object with `properties`; `simpleType` restrictions →
  `enum` or `pattern`; `xs:string`/`xs:integer` → matching JSON primitive types;
  `maxOccurs="unbounded"` → array with `items`; `minOccurs`/`maxOccurs` preserved as constraints;
  `xs:sequence` documented as property order only (JSON objects stay unordered).
- **Attributes:** converted into ordinary JSON properties, conventionally written with an `@`
  prefix, and added to `required` when `use="required"`.
- **Surface:** three tabs — XSD → JSON Schema, JSON → JSON Schema, and XSD → JSON *sample data*.
- **FAQ themes:** schema-from-JSON, complex-type conversion, price, mock data, Draft-07
  compatibility, wiring the output into validators such as ajv.
- **Limits:** none stated; emphasises no uploads / client-side processing.

### 2. jsonviewertool.com — XSD to JSON (browser inspector)

- **Controls:** "include annotations" checkbox, "include namespaces" checkbox, pretty-print
  toggle, an output-mode dropdown (structured object model vs a flat index of schema paths),
  an indent-size dropdown, a built-in sample-schema selector (two presets), plus upload / load
  sample / convert / copy / download buttons.
- **XSD coverage claimed:** complex and simple types, attributes, element declarations,
  restrictions with enumeration values and facets, `minOccurs`/`maxOccurs`, sequence / choice /
  group, extension and derived types, namespaces and `targetNamespace`, import / include /
  redefine references, and optional documentation annotations.
- **Important caveat:** its output is explicitly *not* a JSON Schema document — it is an
  inspection view of the XSD. So it is a competitor for the query, not for the output contract.
- **Limits stated:** external imports are not fetched; very large schemas get slow; substitution
  groups, unions, and wildcards are left for the user to interpret; namespace prefixes vary.

### 3. Oxygen XML Editor — XSD to JSON Schema converter (desktop, add-on)

- Ships as a menu action inside a commercial XML IDE, behind an add-on install. The public doc
  page documents the entry point only and defers all option/limitation detail to the add-on's own
  documentation, so no option list could be confirmed from the public page.
- Useful signal anyway: the conversion is positioned as one action inside a larger JSON-tools
  suite (schema editing, validation, sample generation) — i.e. a workflow product, not a
  single-purpose page. gizza competes on the single-purpose page.
- Neighbouring products in the same class (JetBrains plugin, Altova XMLSpy) sit in the same
  bucket: IDE/desktop integration, licensed, no browser surface.

## Table stakes → decision

Every table-stake below lands in the descriptor or in the out-of-model list; none is dropped
silently.

| Table stake (seen at ≥1 competitor) | Fit | Where it lands |
| --- | --- | --- |
| `complexType` → object + `properties` | in-model | core, always on |
| `xs:*` builtin → JSON type (+ `format`) | in-model | core, always on |
| Facets → `enum`, `pattern`, length/range/`multipleOf` | in-model | core, always on |
| `minOccurs`/`maxOccurs` → `required` + array `minItems`/`maxItems` | in-model | core + `required_from_occurs` param |
| Attributes as properties, `use="required"` → required | in-model | core + `attribute_prefix` param |
| `@` attribute-name prefix convention | in-model | `attribute_prefix` param, default `@`, empty disables |
| `xs:annotation`/`xs:documentation` → `description` | in-model | `annotations` param (default on) |
| Draft choice | in-model | `draft` param — we offer **both** 2020-12 and draft-07 (competitor 1 offers draft-07 only) |
| Root selection | in-model | `root_element` param (default: first global element) |
| `xs:sequence` / `xs:all` | in-model | core, always on |
| `xs:choice` | in-model | core — emitted as a real `oneOf` required-set constraint, which the inspector-style competitor leaves to the reader |
| `complexContent`/`simpleContent` `extension` + `restriction` | in-model | core, always on (base members merged; `simpleContent` text goes to `text_property`) |
| Global `simpleType`/`complexType` reuse, `ref=` | in-model | core — emitted as `$ref` into `$defs`/`definitions`, pruned to what the root reaches |
| Strict vs open objects | in-model | `additional_properties` param (default strict) |
| Namespace prefix handling | in-model | core — matching is by local name, so any prefix binding for the XSD namespace works |
| Preset sample schemas / one-click examples | in-model | three `[[example]]` chips in `meta.toml` |
| Copy / download / reset buttons | in-model | the generator already gives every `format = "text"` page Copy, Download and Reset |
| Pretty-print + indent-size dropdown | **considered, rejected** | output is always pretty-printed 2-space JSON; an indent dropdown is schema bloat for a value users can reformat in one click with an existing gizza formatter |
| "Flat index" inspection output mode | **considered, rejected** | that competitor's mode is explicitly *not* JSON Schema; this tool's contract is a valid schema document |
| XSD → sample/mock XML or JSON data | out-of-model (scope) | a different tool, not a converter option |
| JSON → JSON Schema tab | out-of-model (scope) | already covered by the existing `json-to-json-schema` block |
| Resolving `xs:import` / `xs:include` / `xs:redefine` from URLs | **out-of-model** | gizza blocks are browser-local with no fetch on this surface; the tool converts what you paste and reports the unresolved reference instead of guessing |
| Substitution groups | **out-of-model** | needs the full substitution-group graph across imported schemas; the head element still converts normally |
| XSD 1.1 assertions (`xs:assert`), conditional type assignment | **out-of-model** | XPath evaluation has no JSON Schema equivalent |
| File upload of an `.xsd` | **considered, rejected** | pure text tools in this repo take pasted text; paste covers the case without a second input surface |

## UX control patterns adopted

- `multiline = true` textarea for the XSD paste (competitors all use a large left editor).
- `[input.labels]` friendly labels on the draft `<select>`.
- Three `[[example]]` preset chips (order-style schema, facets/enumeration schema, attributes +
  choice schema) — the declarative answer to competitor "load sample" buttons.
- Real placeholders on every text field; limits and edge cases stated in the page copy, which no
  scanned browser competitor does beyond a one-line caveat.

## Differentiators we ship

1. Both Draft 2020-12 **and** Draft-07 (competitor 1 is draft-07 only).
2. `xs:choice` becomes an actual `oneOf` constraint rather than a flattened bag of properties.
3. Reachability-pruned `$defs`/`definitions` instead of dumping every global type.
4. Named, line-context error messages for unsupported constructs instead of silent mis-conversion.
5. Same converter available on the page, the CLI, and the chat schema from one descriptor.
