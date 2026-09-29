# xml-structure-analyzer — competitor analysis (2026-09-24)

Scan run **before** implementation, per `create-next-tool` step 4. Paraphrase only — no
competitor copy, branding, or trademarks are reproduced, and no competitor wording was
carried into `page/`.

Backlog row: `xml-structure-analyzer` — *"Reports the element tree, tag counts, max depth, and
attribute usage of an XML document."* Type hint `pure`; loader feasibility note in the CSV:
"plain Rust→WASM: a streaming XML parser tallying elements and depth runs offline — via
quick-xml crate."

## Dup check (why this is a new block, not an enhancement)

`ls blocks/ | grep -i xml` → `csv-to-xml`, `json-to-xml`, `xml-diff`, `xml-formatter`,
`xml-namespace-stripper`, `xml-to-csv`, `xml-to-json`. None reports document *shape*:

- `xml-formatter` — pretty/minify + well-formedness only (`format(xml, mode, indent)`); no
  counts, no depth, no attribute inventory.
- `xml-to-json` / `xml-to-csv` — whole-document **transcodes**; they emit converted data, not
  structural statistics (xml-to-csv flattens one repeating record type into columns).
- `xml-namespace-stripper` — rewrites namespace prefixes out of the markup.
- `xml-diff` — compares two documents.
- `xpath-query` — selects nodes by an expression the user must already know; this tool exists to
  tell you the shape *before* you can write that expression (the CSV's own use case).

The direct sibling is **`blocks/json-structure-analyzer`** (depth / key frequency / per-path
types / array stats for JSON). This is the XML counterpart of that block, on a different
parser and with XML-specific dimensions JSON has no analogue for (attributes, namespaces,
text vs. CDATA vs. comments vs. processing instructions, the XML declaration and DOCTYPE).
Descriptor shape and report style deliberately mirror the JSON block for cross-tool
consistency.

Also checked the skiplist: the XML entries there (`xml-validator`, `xml-prettify`,
`xml-beautify`, `xml-escape-unescape`, `xml-numeric-value-extractor`, `xpath-extractor`) are
all about validation, reformatting, escaping, or value extraction — none is a structure
report, and none points at this slug.

## Competitors reviewed

Five candidates surfaced; four were reachable and reviewed in depth.

1. **MiniWebtool — XML Validator** (`miniwebtool.com/xml-validator/`) — validator first, with a
   "Document Statistics" panel: element count, attribute count, maximum nesting depth. Parse
   errors carry line **and** column plus a jump-to-error affordance. Ships three quick-example
   presets (bookstore, config file, an RSS feed containing a deliberate error). Format / Clear
   buttons on the editor.
2. **WSDL-Analyzer — XML Analyzer** (`wsdl-analyzer.net/xml-analyzer/`) — element / attribute /
   text-node counts, maximum nesting depth, every namespace with its prefix, document size, and
   a "structural warnings" health section. Paste-only, one Analyze button, no parameters.
3. **AIFreeForever — XML Viewer / XML Checker** (`aifreeforever.com/tools/xml-viewer`,
   `/xml-checker`) — the widest metric set of the four. A node-counter panel reports total
   elements, total attributes, text nodes, comments, maximum nesting depth, line count, file
   size, and the root element name; the sibling checker adds tag frequency, declaration-field
   checks, namespace inspection, and maximum **and average** depth with level-by-level counts.
   Paste / upload / drag-drop input; Copy, Download, Clear actions.
4. **ToolXML — XML Structure Visualizer** (`toolxml.com/xml-structure-visualizer/`) — the tree
   surface: an interactive collapsible element tree, attributes rendered next to tag names with
   a **toggle to hide them**, node-type identification, element count and depth analysis. Paste
   or upload (states a 500 MB input ceiling). Export as a static image; copy/download. Points
   users at a separate path-frequency tool for aggregate analysis.
5. **OpenFormatter — XML Parser** (`openformatter.com/xml-parser`) — listed in search results as
   reporting root tag, total element count, a unique-tag list, and an XML→JSON tree with `@`
   attribute prefixes; not opened in depth because it overlaps (1)–(4) entirely and its
   transcode half is already `blocks/xml-to-json`.

## Table-stakes matrix

Every table-stake below lands in the descriptor/report or in the out-of-model list. Nothing is
dropped silently.

| # | Capability | Seen in | Fit | Where it landed |
|---|---|---|---|---|
| 1 | Well-formedness check with error position | 1, 2, 3 | in-model | `well_formed` + parse error carrying byte offset **and** line:column (computed from `Reader::buffer_position()`) |
| 2 | Total element count | 1, 2, 3, 4, 5 | in-model | `counts.elements` |
| 3 | Distinct/unique tag list | 3, 5 | in-model | `counts.unique_tags` + the `tags` table |
| 4 | Total attribute count | 1, 2, 3, 4 | in-model | `counts.attributes`, `counts.unique_attributes` |
| 5 | Maximum nesting depth | 1, 2, 3, 4, 5 | in-model | `max_depth` (root element = depth 1) |
| 6 | **Average** depth | 3 | in-model | `avg_depth` (mean depth over element nodes, 2 dp) |
| 7 | Level-by-level element counts | 3 | in-model | `depth_histogram` (one row per level) |
| 8 | Element tree (hierarchy view) | 4, 5 | in-model (static) | `tree` — distinct element **paths** collapsed into a nested tree with per-node occurrence counts, rendered with box-drawing connectors in text mode |
| 9 | Depth-limit / collapse control on the tree | 4 (interactive) | in-model (as a cap) | `tree_depth` param (0 = full tree) — the declarative equivalent of collapsing |
| 10 | Attributes shown beside tag names, with a hide toggle | 4 | in-model | `show_attributes` boolean (default on) — drives tree annotations, the `attribute_usage` table, and the CSV attribute column |
| 11 | Attribute usage / which attributes on which elements | 1, 3, 4 | in-model | `attribute_usage` — per attribute name: occurrences, the elements carrying it, distinct-value count |
| 12 | Tag frequency ranking | 3, 5 | in-model | `tags` table sorted by count desc (cap: `top_tags`, 0 = all) |
| 13 | Namespaces with prefixes | 2, 3 | in-model | `namespaces` (every `xmlns`/`xmlns:p` declaration, deduped) |
| 14 | Text-node count | 2, 3 | in-model | `counts.text_nodes` (non-whitespace only) |
| 15 | Comment count | 3 | in-model | `counts.comments` |
| 16 | CDATA / processing-instruction counts | 4 (node-type ID) | in-model | `counts.cdata_sections`, `counts.processing_instructions` |
| 17 | XML declaration field inspection | 3 | in-model | `declaration` (version / encoding / standalone) |
| 18 | DOCTYPE reporting | 3 | in-model | `doctype` |
| 19 | Root element name | 3, 5 | in-model | `root` |
| 20 | Document size (bytes) + line count | 2, 3 | in-model | `size.bytes`, `size.lines` |
| 21 | Empty / leaf element counts, widest element | — (our addition) | in-model | `counts.empty_elements`, `counts.max_children` |
| 22 | Structural warnings / health report | 2, 3 | in-model | `warnings` (deep nesting, mixed content, unused namespace prefixes, empty elements, truncation notices) |
| 23 | Quick-example presets | 1 | in-model | three `[[example]]` preset chips (bookstore catalog, namespaced SOAP-style envelope, RSS feed) |
| 24 | Copy / Download the report | 3, 4 | in-model (platform) | the generator gives `format = "text"` pages a Download link for free |
| 25 | Multiple report formats (structured + tabular) | — (our addition) | in-model | `format` = `text` \| `json` \| `csv` (CSV = one row per distinct tag, for spreadsheet triage) |

### Out of model — listed, not built

- **Interactive collapsible tree widget** with click-to-expand nodes (ToolXML, AIFreeForever).
  A gizza pure page is a form → one text/JSON output pane; there is no per-node JS tree
  component. Closed as far as the model allows by #8 + the `tree_depth` cap (#9), which gives
  the same "show me only the top N levels" outcome declaratively.
- **Static-image export of the tree** (ToolXML). Page output is text; image output would need
  `build_media_envelope`, which has no page render mode for pure tools.
- **Search / filter within the tree** (ToolXML, AIFreeForever). Interactive-only; node
  selection by expression is already `blocks/xpath-query` and `blocks/css-select-extract`.
- **File upload / drag-and-drop of an .xml file** (AIFreeForever, ToolXML's 500 MB ceiling).
  Pure blocks are `Input::None` + text params; the page file-input widget is ffmpeg-runtime
  only. Paste is the input surface, and the whole document is parsed locally either way.
- **Syntax-highlighted editor with line numbers / jump-to-error** (MiniWebtool,
  AIFreeForever). Editor chrome, not a compute capability — but the *useful* half is kept: the
  parse error reports `line:column` so the user can jump there in their own editor.
- **XSD / schema validation.** Already skiplisted repo-wide (`xml-validator`): no
  wasm-instantiable XSD validator exists for this runtime.
- **Auto-format / prettify the input.** Deliberately out of scope — that is
  `blocks/xml-formatter`; this block never rewrites the document.

## Decisions taken into the descriptor

- Five params, all with `.describe()`: `xml` (required), `format` (`Param::enumv`
  text/json/csv), `tree_depth` (0 = full), `top_tags` (0 = all), `show_attributes` (boolean,
  default true). The two caps mirror `json-structure-analyzer`'s `top_keys`/`top_paths` so the
  two analyzers feel like one family.
- Default `format = "text"` (not `json` as in the JSON block): every competitor's primary
  surface is a human-readable stats panel plus a tree, and the box-drawing tree is the
  headline output here. `json` stays available for piping.
- **Tag names keep their prefixes** as written (`soap:Envelope`, not `Envelope`), because
  prefix usage is part of the structure being reported — unlike `xml-to-csv`, which strips to
  local names for column naming. Namespaces are reported separately so both views are present.
- Paths are collapsed by element name, the direct analogue of the JSON block's `[]`
  index-collapse: every `<book>` under `<catalog>` shares the path `catalog/book`, so a
  1000-entry feed still summarizes to a readable tree.
- No cap on input size: parsing is a single streaming pass (quick-xml pull parser), linear in
  input, and runs entirely in the browser/CLI. Report caps are on *output list length* only,
  and each one sets an explicit truncation flag plus a warning rather than silently cutting.
