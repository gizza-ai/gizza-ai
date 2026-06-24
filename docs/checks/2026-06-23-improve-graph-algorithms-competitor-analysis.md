# graph-algorithms — competitor analysis (2026-06-23)

Tool: `blocks/graph-algorithms` — pure-compute tool. Takes a text edge list and
runs BFS, DFS, shortest path (Dijkstra / unit-weight BFS), topological sort, or
cycle detection. Directed/undirected and weighted/unweighted. Three surfaces
(chat skill, CLI, standalone page) single-sourced from `descriptor()`.

All notes below are **paraphrased** — no competitor copy, branding, or assets
were reproduced.

## Competitors surveyed

1. **VisuAlgo (visualgo.net)** — academic algorithm-visualization site. Separate
   modules for DFS/BFS traversal and single-source shortest paths (BFS,
   Dijkstra, Bellman-Ford, DP on a DAG). Strengths: step-by-step animation,
   pseudo-code highlighting, topological-sort and bipartite-check variants,
   curated example graphs, multiple input modes (draw, adjacency matrix/list,
   edge list). Pedagogy-first; not an API/CLI.
2. **Dev-Toolbox Graph Visualizer (dev-toolbox.tech)** — free in-browser tool.
   Supports BFS, DFS, Dijkstra, and topological sort with step-by-step view;
   shows the graph as an adjacency matrix or adjacency list. Closest feature
   match to ours.
3. **VastlyWise Graph Traversal Visualizer** — interactive canvas for DFS/BFS on
   a graph you build by adding/moving/deleting nodes and edges, then animating
   the traversal. Traversal-only (no shortest path / topo / cycle).
4. **See-Algorithms BFS Visualizer** — draw-your-own-graph BFS, emphasising the
   level-by-level shortest-path-on-unweighted property. Single algorithm.
5. **ToolWaves Pathfinding Visualizer (Dijkstra & A\*)** — grid-based pathfinding
   playground (walls/weights on a grid), Dijkstra + A\*. Grid metaphor, not a
   general edge-list graph; no traversal/topo/cycle reporting.

## Capability diff (theirs → ours)

| Capability | Competitors | Ours | Status |
| --- | --- | --- | --- |
| BFS / DFS traversal | VisuAlgo, Dev-Toolbox, VastlyWise, See-Algorithms | yes | covered |
| Shortest path (Dijkstra) | VisuAlgo, Dev-Toolbox, ToolWaves | yes (Dijkstra + unit-weight BFS) | covered |
| Topological sort | VisuAlgo, Dev-Toolbox | yes (Kahn's, DAG-only with clear error) | covered |
| Cycle detection | VisuAlgo (implicit via topo/bipartite) | yes (directed + undirected, returns an example cycle) | covered — a differentiator; few list it as a first-class algorithm |
| Directed vs undirected | most | yes (explicit flag) | covered |
| Weighted edges | VisuAlgo, Dev-Toolbox, ToolWaves | yes (non-negative, validated) | covered |
| Flexible text input (`->`, `-`, `,`, space, weights, comments, isolated nodes) | partial (most are draw/matrix) | yes | covered — strong; edge-list-first is convenient for copy/paste |
| Deterministic, reproducible output | n/a (animations) | yes (sorted adjacency → stable order) | differentiator for a CLI/API tool |
| API / CLI surface | none (all are visual web apps) | yes (chat skill + `gizza tool`) | differentiator — none of the competitors expose a programmatic surface |
| Step-by-step animation / visual graph render | VisuAlgo, Dev-Toolbox, VastlyWise, See-Algorithms | no (text report only) | gap — see below |
| Adjacency matrix / list view | VisuAlgo, Dev-Toolbox | no | gap (out of current model scope) |
| A\* / Bellman-Ford / MST / SCC | VisuAlgo (Bellman-Ford), ToolWaves (A\*) | no | considered, not built |

## Gaps considered and decision

**In-model and built this run** — the tool already ships a comprehensive set:
five algorithms, directed/undirected, weighted/unweighted, forgiving edge-list
parsing (multiple separators, comments, isolated nodes, weights), validated
errors (unknown algorithm, missing node, negative weight, empty graph,
topo-on-undirected), and deterministic output. Each algorithm + error path has a
unit test (16 core tests), a drift-guard schema test, six wafer fixtures, and
five Playwright page tests incl. a query-param deep-link.

**Gaps deliberately not built (out of current page/tool model):**
- **Visual graph rendering / step animation.** Every competitor is a visual app;
  our pages render a text result (`format = "text"`). A canvas render or
  per-step animation is a different page primitive than the current
  input-fields→text-output model and would be a much larger change. Our
  differentiator is instead the **programmatic** surface (chat + CLI) and
  deterministic text output that none of the visual competitors offer. Listed,
  not forced in.
- **Adjacency-matrix / adjacency-list display.** A presentation feature tied to
  the visual model; not built. The text report already names every node and the
  reachable count.
- **More algorithms (A\*, Bellman-Ford, MST/Kruskal/Prim, strongly-connected
  components).** Genuinely useful future additions and all pure-Rust-feasible,
  but each is a distinct algorithm with its own semantics, tests, and schema enum
  entry — out of scope for a single build. Bellman-Ford specifically would let us
  accept negative weights (which we currently reject), a reasonable follow-up.

## Result

graph-algorithms ships feature-complete against the in-scope (text-in / text-out,
no-canvas) competitor set, and uniquely adds a chat/LLM + CLI surface plus
deterministic output. Visual/animation features are noted as out of the current
page model, not built.

## Sources

- https://visualgo.net/en/dfsbfs
- https://visualgo.net/en/sssp
- https://www.dev-toolbox.tech/tools/graph-visualizer
- https://vastlywise.com/Graph-Traversal-Visualizer
- https://see-algorithms.com/graph/BFS
- https://toolwaves.com/tools/pathfinding-visualizer
