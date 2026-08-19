# Competitor analysis: mermaid-from-data

Date: 2026-08-16

## Scope

Tool under review: generate Mermaid source from structured nodes and edges for flowcharts, class diagrams and ER diagrams. The gizza model is pure text transformation: no server-side rendering, no proprietary diagram editor, no hosted storage.

Search query used: `Mermaid diagram generator from CSV nodes edges online`.

## Competitor / reference scan

| Source | What it provides | Table-stakes found | In-model decisions for this tool | Out-of-model / not shipped |
| --- | --- | --- | --- | --- |
| ToDiagram | Search result describes importing JSON, YAML, XML, CSV and Mermaid into interactive diagrams, plus custom node/edge diagrams. | Data-to-diagram workflow, CSV support, custom nodes/edges, visual inspection. | Accept delimited edge rows, optional node metadata and multiple diagram shapes from structured data. | Interactive layout canvas and visual editing are outside the plain-text block model. |
| Mermaid Flow | Search result describes a visual Mermaid live editor for flowcharts with drag-and-drop nodes, edges and labels. | Flowchart output, node labels, edge labels, immediate editor-friendly Mermaid source. | Emit flowchart syntax with node labels, shapes, directions, edge labels and edge styles. | Drag-and-drop editing and browser renderer controls are not part of this generator. |
| Mermaid Live Editor | Official live editor for Mermaid's markdown-like diagram language. | Pasteable Mermaid source, live preview, broad diagram syntax, markdown/documentation focus. | Return plain Mermaid source, optionally wrapped in a Markdown `mermaid` fence for paste targets. | Live preview/rendering is deferred to existing Mermaid renderers. |
| Mermaid documentation | Official syntax reference for flowcharts, class diagrams, ER diagrams and relationship operators. | Diagram type selection, flowchart directions, class relationships, ER cardinalities, labels and titles. | Support `flowchart`, `class` and `er`; map class/ER relationship style words to Mermaid operators; add optional title front matter. | Full Mermaid syntax coverage (sequence, state, gantt, sankey, pie, etc.) is out of scope for this backlog item. |

## Capability decisions

Built in-model:

- Arrow-line input such as `Start -> Build : label`.
- Delimited edge rows with auto, pipe, comma or tab delimiters.
- Chain mode for path rows where adjacent columns are linked.
- Optional node declarations for labels, flowchart shapes/groups, class members and ER attributes.
- Diagram enum: flowchart, class diagram and ER diagram.
- Direction enum: TD/TB/LR/BT/RL.
- Flowchart node-shape and edge-style enums.
- Class and ER relationship style names mapped to Mermaid operators/cardinalities.
- Optional title and optional Markdown fenced output.
- Hard caps for size, nodes and edges, with explicit errors.

Out of model / deferred:

- Rendering diagrams to SVG/PNG, because the tool emits source and browser rendering would add a different output surface.
- Drag-and-drop editing, because this public toolkit block is a deterministic text transform rather than a canvas application.
- Full Mermaid grammar support across every diagram family, because the backlog item focuses on nodes and edges.
- Layout tuning beyond Mermaid direction and syntax, because Mermaid renderers handle layout.

## UX/control decisions

- `edges` and `nodes` are multiline text areas to support pasted tables and arrow lists.
- Fixed choices use enum controls for diagram type, direction, row mode, delimiter, default shape and default edge style.
- `fence` is a checkbox because it toggles Markdown wrapping.
- Example chips cover flowchart, class, ER and chain workflows.

## Verification notes

The descriptor includes every in-model table-stake. CLI and page tests can assert exact Mermaid source, query-param deep links and non-default enum/checkbox states without requiring a Mermaid renderer.
