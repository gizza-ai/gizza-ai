# dot-to-mermaid — competitor analysis (2026-08-17)

## Sources reviewed

| Source | What it covers | Notes for our implementation |
| --- | --- | --- |
| DevToolsDaily Graphviz to Mermaid Converter | Browser text-area conversion from Graphviz/DOT to Mermaid flowcharts; public FAQ says it focuses on nodes and directed edges and may ignore Graphviz-only styling. | Table stakes: paste DOT, immediate Mermaid output, clear statement that not all styling maps. Our tool includes these, plus controls for direction, labels, shapes, clusters, colors and warnings. |
| r3code/dot2mermaid | Small open-source converter whose README positions it as DOT to Mermaid flowchart conversion. | Table stakes: CLI/library style conversion, Mermaid flowchart output. Sparse documented options, so our tool should expose common mapping toggles and worked examples. |
| dot2mermaid on PyPI | Python package that depends on Graphviz/code2flow/pygraphviz, exposes a parser and README-markdown insertion flow, and supports color classes in examples. | Table stakes: programmatic conversion, markdown-friendly output, color/style handling. Our in-model fit: browser/CLI single conversion with optional code fence and style/linkStyle lines. Out of model: crawling a code directory via code2flow. |

## Table-stakes decisions

| Capability / UX pattern | In model? | Decision |
| --- | --- | --- |
| Paste DOT source and return Mermaid flowchart text | Yes | Required `dot` text field, multiline page input, text output. |
| Preserve direction (`rankdir`) | Yes | `direction=auto` follows DOT `rankdir`; explicit TD/LR/BT/RL enum overrides it. |
| Map node labels and common shapes | Yes | `shapes` checkbox defaults on; supports box/ellipse/circle/doublecircle/diamond/hexagon/cylinder/component/parallelogram/trapezium fallbacks. |
| Map edge labels and simple edge styles | Yes | `edge_labels` and `link_styles` checkboxes default on; dashed/dotted/bold/invisible/arrowless/bidirectional/back edges are mapped where Mermaid can express them. |
| Convert clusters/subgraphs | Yes | `subgraphs` checkbox defaults on; cluster labels become Mermaid subgraph titles. |
| Preserve colors/styles where possible | Yes | `colors` checkbox emits Mermaid `style` and `linkStyle` statements for simple colors/pen widths. |
| Warn about lossy Graphviz-only constructs | Yes | `warnings` checkbox defaults on and emits `%%` notes for ports, unknown shapes, color lists, flattening, etc. |
| Markdown/README-friendly output | Yes | `fence` checkbox wraps in a ```mermaid code fence; examples include a fenced preset. |
| Worked examples / presets | Yes | Four example chips: decision flow, clustered services, undirected graph, styled fenced README. |
| Convert Mermaid back to DOT | No | Opposite direction is a separate tool scope. |
| Render the graph visually | No | This toolkit produces source text; rendering belongs to Mermaid/Graphviz viewers. |
| Crawl a source-code directory and generate DOT first | No | Requires filesystem/project analysis and package-specific semantics; PyPI package's code2flow workflow is out of this tool's pure text-conversion model. |

## Verification focus

- Exact CLI/page output for labelled directed graph with `rankdir=LR`.
- Deep-link query parameters for a fenced/title output.
- Matrix for direction override, checkboxes off, cluster conversion, undirected graphs, styles, color output and invalid DOT errors.
