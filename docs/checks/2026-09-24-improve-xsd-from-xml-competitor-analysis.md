# xsd-from-xml — competitor analysis (2026-09-24)

Scan run BEFORE implementing `blocks/xsd-from-xml`, per `/improve-tool` Phase 2–3. Everything
below is **paraphrased** from public product/tool pages and reference documentation — no
competitor copy, branding, logos, or trademarks are reproduced. The goal is table-stakes
parameter coverage, not imitation.

## Competitors profiled

### 1. inst2xsd (Apache XMLBeans command-line schema inferrer)
- URL: `https://xmlbeans.apache.org/docs/2.0.0/guide/tools.html`
- Features: infers one or more `.xsd` documents from one or more sample instance documents;
  optional round-trip validation of the generated schema against the inputs.
- Params/options (with documented defaults):
  | name | type | default | range |
  | ---- | ---- | ------- | ----- |
  | design | choice | venetian blind | russian doll / salami slice / venetian blind |
  | simple-content-types | choice | smart | smart / string |
  | enumerations | integer or "never" | 10 | max distinct values that become an enumeration |
  | outDir | path | current dir | — |
  | outPrefix | string | `schema` | — |
  | validate | flag | off | — |
- Input formats: XML instance documents. Output: `.xsd` files on disk.
- Output quality: "smart" mode narrows numeric text to the smallest fitting builtin
  (documented example: `xs:byte`), which is frequently *too* narrow for real data.
- UX patterns: CLI only, multi-document input, batch output.
- Limits: none stated beyond available memory.
- Free vs paid: open source.

### 2. Liquid Technologies online XML→XSD converter
- URL: `https://www.liquid-technologies.com/online-xml-to-xsd-converter`
- Features: browser form that infers an XSD from a pasted/uploaded sample, with a live
  document-validity indicator and a structured error table (severity / location / file /
  message).
- Params/options: "occurrence" as restricted vs relaxed; "type inference" as restricted vs
  relaxed; indent character (space or tab); indent depth; indent-attributes toggle. Defaults
  are not stated on the page.
- Input formats: a single XML document. Output: one XSD document.
- UX patterns: real-time validity feedback, tabular error reporting, an explicit
  data-handling acknowledgement checkbox, and an online-vs-desktop feature comparison.
- Limits: online edition stated as under 512 KB per document and one schema document out;
  the desktop edition is pitched as unlimited.
- Free vs paid: online tier free and capped; full capability sold as a desktop product.

### 3. RAKKOTOOLS XML Schema (XSD) Generator
- URL: `https://en.rakko.tools/tools/52/`
- Features: paste-or-upload XSD generation that runs entirely client-side (the page states
  input is not sent to a server).
- Params/options: output design selectable across russian doll / salami slice / venetian
  blind; a "smart" switch that either infers types from values or declares every element as
  a string type.
- Input formats: pasted XML or an uploaded file. Output: XSD text.
- UX patterns: in-browser processing as an explicit privacy claim; a small fixed option set
  rather than a long form.
- Limits: not stated on the page.
- Free vs paid: free.
- (Profile assembled from the search-result description; the page itself returned HTTP 429
  to the fetcher during this scan, so it was not read directly. Recorded honestly rather
  than dropped.)

### 4. Tooltube XSD Generator
- URL: `https://www.tooltube.in/xsd-xml-schema-generator`
- Features: paste or upload XML, generate XSD, clear-all reset; automatic structure analysis
  and data-type inference.
- Params/options: file-encoding dropdown; XSD design across the same three patterns, each
  with a one-line explanation of what it does to element/type placement.
- Input formats: pasted XML or upload. Output: XSD text.
- UX patterns: explains each design choice inline next to the control instead of assuming
  the user knows the pattern names; explicit Clear/Reset action.
- Limits: none stated. No FAQ section.
- Free vs paid: free.

### 5. Site24x7 XML to XSD Converter
- URL: `https://www.site24x7.com/tools/xml-to-xsd.html`
- Features: browse a local `.xml`, generate, save the `.xsd`.
- Params/options: the three design patterns only.
- Input formats: file browse. Output: downloadable XSD.
- UX patterns: minimal single-purpose page; download-the-result as the primary action; the
  tool page is a funnel for a separate monitoring product.
- Limits: not documented, but the endpoint rate-limits ("too many requests") — it is a
  server-side conversion, not browser-local.
- Free vs paid: free.

### Cross-cutting observation from the search sweep
Several tool pages (and the general write-ups around them) state plainly that an inferred
XSD is a **draft**: a single sample cannot prove optionality, true repetition bounds, value
domains, or domain constraints. That framing is honest and worth adopting in our own copy —
as our own words, and backed by options that let the user choose how tight the guess is.

## Gap list vs the planned gizza tool (tagged in-model / out-of-model)

| # | Gap (≥1 competitor ships it) | Dimension | Tag | Decision |
| - | ---------------------------- | --------- | --- | -------- |
| 1 | Three schema design patterns (russian doll / salami slice / venetian blind) | capabilities | in-model | **Built** — `design`, default `venetian-blind` (matches the inst2xsd default). |
| 2 | Smart type inference vs everything-as-string | capabilities | in-model | **Built** — `type_inference` = `smart` \| `string`. |
| 3 | Enumeration inference with a distinct-value cap | capabilities | in-model | **Built** — `enumerations` integer cap. Default `0` (off), not 10: a *single* sample makes enum inference over-constraining, so ours is opt-in and says so. |
| 4 | Restricted vs relaxed occurrence inference | capabilities | in-model | **Built** — `occurrence` = `restricted` \| `relaxed`; also drives attribute `use`. |
| 5 | Indent depth control | ux/capabilities | in-model | **Built** — `indent` 0–8 spaces. |
| 6 | Indent character (space vs tab) | ux | in-model | **Considered, rejected** — one more enum for a cosmetic axis; `indent = 0` already gives compact output and XSD consumers ignore whitespace. Noted here rather than silently dropped. |
| 7 | XML declaration on the output | capabilities | in-model | **Built** — `declaration` boolean, default true. |
| 8 | targetNamespace handling | capabilities | in-model | **Built** — auto-detected from the sample's default namespace, overridable via `target_namespace`; sets `elementFormDefault="qualified"` when namespaced. No competitor exposed this; it is a real correctness gap in their output for namespaced samples. |
| 9 | Multi-document / batch input (several instance files merged into one schema) | capabilities | in-model in principle, **rejected for now** | Our surface takes one text field; merging N documents would need a multi-file input this repo's page form doesn't have. A user can concatenate samples under a synthetic root — stated on the page. |
| 10 | Round-trip validation of the generated schema against the sample | capabilities | out-of-model (scope) | Would mean shipping a full XSD validator alongside the generator — that is the sibling direction, not this tool. Listed, not built. |
| 11 | File upload of the `.xml` | ux | out-of-model here | Pure tools take a text field; browser-local paste covers the case. |
| 12 | "Not sent to a server" privacy claim | copy | in-model | **Built** — our whole model is browser-local wasm; the page says so in our own words. |
| 13 | Inline explanation of each design pattern next to the control | copy/ux | in-model | **Built** — friendly `[input.labels]` on the design select plus a dedicated content section and FAQ entry. |
| 14 | Error table with severity/location | ux | partially in-model | **Built as actionable single errors** — parse failures name the byte position and what was expected. A multi-row error grid needs a validator, not an inferrer. |
| 15 | Stated input size limit | copy | in-model | **Built** — 1,000,000 byte cap, stated on the page and in the descriptor. |

Beyond table stakes (no competitor scanned ships these; added because a single-sample
inferrer is otherwise wrong on real documents):

- **Unordered-content detection** — when sibling order varies between repeats of the same
  parent, a plain `xs:sequence` would reject the very document it was inferred from. We
  detect that and emit a repeating `xs:choice` instead.
- **Mixed content** — an element with both text and child elements gets `mixed="true"`.
- **`xsi:nil` → `nillable="true"`**, and `xsi:*` attributes are excluded from the inferred
  attribute list rather than declared as real attributes.
- **Foreign-namespace children** are represented with an `xs:any` wildcard in a repeating
  choice, so the generated schema still accepts the sample instead of silently producing an
  unusable single-namespace schema.
- **Numeric widening** — integers map to `xs:int`/`xs:long`/`xs:integer` by observed range
  rather than to the narrowest builtin; a two-digit sample value should not permanently
  constrain the field.

> Original work only — no competitor copy, branding, or trademarks were copied.
