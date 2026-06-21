# xml-to-csv — competitor analysis (2026-06-21)

New tool: flatten repeated XML elements into a CSV table, columns inferred from
child tags and attributes. Runs fully in-browser (pure-Rust wasm); no upload, no
account, no AI model.

## Surfaces verified

- **Chat block:** `cargo test --workspace` (19 core + drift-guard schema test) green;
  `wafer build` validates + instantiates the wasm32-wasip1 block (381.9 KiB).
- **CLI:** `gizza tool xml-to-csv …` — auto-detect, explicit record, attributes
  on/off, tab/semicolon/pipe delimiters, nested dot-notation, and the no-records
  error path all confirmed.
- **Page:** Playwright (3 specs) — auto-detect + attributes, attributes-off with
  explicit record + tab delimiter, and a query-param deep-link pre-fill — all pass.

## Competitors surveyed (paraphrased; no copy/branding reused)

Surveyed the common online XML→CSV converters (data.page, onlinexmltools.com,
text2csv.com, conversiontools.io, several "xmltools"/"jsontotable" clones). Shared
feature set:

- Pick / auto-detect the repeated record element; one record → one CSV row.
- Attributes preserved as their own columns, commonly with an `@` prefix
  (`@id`, `@category`) to distinguish them from element text.
- Nested elements flattened to **dot-notation** column names (`address.street`),
  or alternatively kept as same-row columns / expanded to extra rows.
- Configurable delimiter (comma / semicolon / tab / custom) and quote handling.
- Many run client-side and never upload data (privacy angle).
- Some offer XLSX as an alternate download.

## Gap diff → our tool

| Capability | Competitors | Ours (after this build) |
|---|---|---|
| Auto-detect record element | yes | yes (most frequent direct child of root) |
| Explicit record tag | yes | yes (`record` param) |
| Attributes as `@`-prefixed columns | common | yes (`@attr` on record, `path@attr` on nested), toggleable |
| Nested → dot-notation columns | yes | **added** (`address.city`); non-leaf elements get no own column |
| Repeated child tags | varies | yes (`tag`, `tag.2`, `tag.3`) |
| Delimiter choice | yes | yes (char or comma/tab/semicolon/pipe) |
| Auto CSV quoting | yes | yes (`csv` crate quotes delimiter/quote/newline) |
| Entity + CDATA decoding | varies | yes |
| Namespaced tags | varies | yes (flattened to local name) |
| Client-side / no upload | some | yes (always; pure wasm) |

### In-model gap closed this run

- **Nested-element dot-notation flattening.** Initial build collapsed nested
  children to concatenated text (`Paris75001` for `<address><city>…<zip>…`).
  Rewrote the record parser to a path-stack walk: a leaf element emits one column
  named by its dot-joined path below the record; a non-leaf element emits none.
  Attributes on a nested element become `path@attr`. Added 2 tests
  (`nested_elements_use_dot_notation`, `deeply_nested_attribute_path`).

## Out-of-model (considered, not built)

- **XLSX output.** Competitors offer an .xlsx download; gizza's page/CLI surface
  renders text/CSV. A binary spreadsheet envelope would be a separate media tool.
- **Expand-nested-to-rows / matrix layouts.** Alternative flattening strategies
  (one row per nested repeated child) change the row cardinality; deferred as a
  potential future `mode` param rather than forced in now.
- **File upload of large XML.** The page takes pasted text (consistent with the
  other text tools); a file-drop input is a framework-level page feature.

## Result

Built + verified across all three surfaces; one in-model capability gap
(dot-notation nesting) found in research and closed with tests.
