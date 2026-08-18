# json-to-graph competitor analysis — 2026-08-18

Backlog item: `json-to-graph` — visualize a JSON document's structure as a node-link graph in Mermaid or Graphviz DOT.

## Sources skimmed

| Competitor | What it exposes | Table-stakes patterns observed | Fit decision |
| --- | --- | --- | --- |
| JSON Master — JSON Visualizer | Interactive JSON/XML/YAML/CSV visualization as node graphs or tree grids with exploration/editing affordances. | Paste/upload data, node-link and tree views, support for nested structures, interactive exploration, large-document handling. | In-model: JSON parsing, node/edge output, structure-preserving traversal, large-document caps. Out-of-model: interactive canvas, editing, multi-format import. |
| JSONViewer.tools — JSON Viewer | A broader JSON workspace with formatting, validation, compare, charts/tables and visual graphs. | Paste JSON, validate before visualizing, graph/table/chart modes, developer-friendly output and examples. | In-model: invalid JSON errors, graph source output, worked examples, options for values and layout. Out-of-model: charts, table inference, side-by-side diff UI. |
| DaivVerse — JSON to Graph | Focused JSON-to-Mermaid tool with clear input/output, data handling and limitation notes. | Mermaid output, JSON input textarea, graph direction/structure emphasis, limitation copy. | In-model: Mermaid `flowchart` output, direction enum, local deterministic transform, limits. |
| Azimutt JSON to DOT converter | Converts JSON-like schema/data into Graphviz DOT output for use with Graphviz tooling. | DOT syntax output, Graphviz-oriented naming, copy/paste source for downstream rendering. | In-model: DOT `digraph` output, `rankdir` direction, escaped labels. Out-of-model: schema/ERD inference beyond raw JSON parent-child structure. |

## Descriptor decisions

- `json` is a required multiline string; the core only accepts strict JSON so parse failures are immediate and deterministic.
- `format` is an enum (`mermaid`, `dot`) because competitors split between Mermaid-friendly docs workflows and Graphviz render pipelines.
- `direction` is an enum (`TD`, `LR`, `BT`, `RL`) to cover both Mermaid layout direction and Graphviz `rankdir`.
- `max_depth`, `max_nodes` and `max_array_items` are bounded integer sliders for large payloads; competitors need some way to keep dense JSON readable.
- `include_values` is a checkbox so users can make docs-safe keys-only diagrams without scalar data.
- `value_max_len` bounds long keys/strings; `show_types` adds type/size hints when the graph is used as a schema/structure map.

## Verification matrix to cover

- Mermaid default output with exact node/edge fragments.
- DOT output with `rankdir` and escaped labels.
- Deep-link query params for DOT/LR, depth cap, array cap, values disabled and types enabled.
- Enum coverage for both formats and all directions.
- Non-default checkbox states: `include_values=false` and `show_types=true`.
- Boundary/error coverage: invalid JSON, `max_nodes=1`, `max_nodes=0` rejected, depth and array elision nodes.

## Deliberately not built

- No interactive graph canvas, drag/pan/zoom, node search, editing, schema inference, image export or hosted sharing; this block emits deterministic diagram source only.
- No JSON5/YAML/XML/CSV import. Those conversions belong to neighbouring tools and would make this pure JSON transformer ambiguous.
- No Mermaid or Graphviz renderer embedded in the block; users paste the source into their renderer of choice.
