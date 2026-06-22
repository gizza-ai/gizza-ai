# adjacency-matrix-converter — competitor analysis (2026-06-22)

Tool: convert a graph between an edge list, adjacency matrix, adjacency list,
incidence matrix, degree matrix, and the graph Laplacian. Pure-Rust, runs in the
chat block / CLI / standalone page; no server, no network.

## Top competitors surveyed

| Tool | Conversions | Directed | Weighted | Labels | Notes |
| --- | --- | --- | --- | --- | --- |
| GetZenQuery — Adjacency Matrix Generator | edge list → adjacency, **incidence, degree, Laplacian** | yes | yes (3rd token) | custom | Closest match; emits the same four matrix forms |
| MiniWebtool — Adjacency Matrix Calculator | adjacency ⇄ edge list ⇄ **adjacency list** | auto-detect | yes | custom labels field | Also: degree sequence, density, components, matrix powers, SVG viz |
| ScientificCalculatorOnline | visual graph ⇄ edge list ⇄ matrix | `A -> B` / `A -- B` | `C -- D : 7` | custom | Live two-way sync, graph drawing |
| VisuAlgo — Graph DS | edge list / adjacency matrix / adjacency list → drawing | yes | yes | indices | Teaching visualizer |
| GeoGebra — Adjacency Matrix to Graph | matrix → graph | — | nonzero cells | — | Spreadsheet-driven, draws the graph |

Sources: getzenquery.com/tools/adjacency-matrix-generator, miniwebtool.com/adjacency-matrix-calculator,
scientificcalculatoronline.io/adjacency-matrix-calculator, visualgo.net/en/graphds, geogebra.org/m/fyg2jpft.

## Gap diff and decisions

Capabilities the leaders have that we initially lacked, ranked by fit-to-model:

1. **Adjacency-list output** (MiniWebtool, VisuAlgo) — pure compute. **CLOSED:**
   added `to=list` (`A: B C`, weighted as `B(3)`).
2. **Degree matrix output** (GetZenQuery) — pure compute. **CLOSED:** added
   `to=degree` (diagonal weighted-degree matrix).
3. **Graph Laplacian output** (GetZenQuery) — pure compute, high value for
   spectral graph theory / spanning-tree counts. **CLOSED:** added
   `to=laplacian` (`L = D − A`, undirected only with a clear error otherwise).
4. **Auto-detect directed/undirected & custom labels** (MiniWebtool) — partially
   present: we auto-detect a label header/column on matrix input and preserve
   first-seen labels from an edge list. Direction stays an explicit flag (cleaner
   and unambiguous for a deterministic tool); not changed.
5. **Comma- and space-separated input, comments, isolated vertices** — already
   supported (tokenizer splits on whitespace/commas, `#` comments, single-token
   isolated-vertex lines).

### Out-of-model (NOT built — deterministic text/CLI tool, no canvas)

- **Interactive SVG graph visualization / drawing** (MiniWebtool, VisuAlgo,
  GeoGebra) — the page renders text output only; a live graph canvas is a
  different surface and out of scope.
- **Live two-way sync between formats** (ScientificCalculator) — our model is a
  single input → single output recompute; not applicable.
- **Derived analytics** (density, connected components, matrix powers, degree
  sequence) — adjacent features, deferred to keep the tool focused on the
  representation conversion it is named for.

## Verification (all three surfaces)

- **Chat block:** `wafer build` validates `target/block.wasm` instantiates (344 KiB OK).
- **Unit + drift:** 23 core tests + 1 descriptor drift-guard test pass.
- **CLI:** `gizza tool adjacency-matrix-converter …` verified for adjacency,
  incidence (directed -1/+1), edges, list, degree, laplacian, and error paths.
- **Page (Playwright):** 5 specs pass — edges→adjacency, edges→incidence
  (directed), adjacency→edges, edges→laplacian, and a query-param deep-link with
  weighted adjacency.
