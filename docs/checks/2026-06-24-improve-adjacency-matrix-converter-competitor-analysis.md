# Competitor Analysis: Adjacency Matrix Converter (2026-06-24)

Analysis of 5 competitor tools to identify feature gaps and improve the `adjacency-matrix-converter` tool.

---

## 1. Competitor Profiles

### Competitor 1: CS Academy Graph Editor
* **URL:** `https://csacademy.com/app/graph_editor/`
* **Features:**
  * Interactive canvas for creating vertices and linking them using point-and-click operations.
  * Real-time text parser that automatically translates lists of connections into node diagrams.
  * Dynamic force-directed arrangement simulation to automatically position vertices cleanly.
  * Operational modes allowing users to draw elements, edit labels/costs, trigger physics, or delete objects.
  * Real-time visual detection and color-coding for connected components, bridges, cut vertices, and minimum spanning trees.
* **Parameters & Options:**
  * `vertex_dimension_radius` (slider, range 5 to 50)
  * `text_label_scale` (slider, range 8 to 36)
  * `optimal_connection_span` (slider, range 30 to 300)
  * `attraction_pull_coefficient` (slider, range 0.1 to 2.0)
  * `inward_centering_gravity` (slider, range 0.0 to 1.0)
  * `one_way_orientation` (directed checkbox)
  * `arced_connection_lines` (checkbox)
  * `render_numeric_costs` (weighted checkbox)
  * `display_network_metadata` (checkbox)
* **Input Formats:** Edge list text, direct mouse/pointer draw.
* **Output Formats:** Plain text edge list, visual browser canvas representation.
* **Output Quality:** High-fidelity interactive physics canvas, no file/vector exports.
* **UX Patterns:** Split-pane layout, force-directed simulation, live-preview.
* **SEO Copy Angles:** Edge list converter, graph simulator, spanning trees visualization.

### Competitor 2: GraphOnline
* **URL:** `https://graphonline.ru/en/`
* **Features:**
  * Interactive network topology sketching and manual node/edge creation on a canvas.
  * Execution of custom script-based algorithms via an integrated code interface.
  * Comprehensive mathematical analysis: path calculations, cycle discovery, network optimization, and topological coloring.
  * Automatic layout restructuring algorithms for tangled nodes.
  * Real-time validation and format repair assistant for matrix and connection list pastes.
* **Parameters & Options:**
  * Node shapes, size, color, and labels.
  * Edge weight, direction, line styling, thickness, and color.
  * Workspace light/dark/high-contrast themes.
* **Input Formats:** Adjacency matrix, incidence matrix, shortest-path distance matrix, edge list, manual canvas draw.
* **Output Formats:** Share link, PNG/JPEG image downloads, matrix/list text files.
* **Output Quality:** High-fidelity text matrices, standard raster graphics.
* **UX Patterns:** Tool-based mode toggling, right-click context menu, theme setting panel.
* **SEO Copy Angles:** Pathfinder simulation, discrete math tools, network diagramming.

### Competitor 3: VisuAlgo Graph DS
* **URL:** `https://visualgo.net/en/graphds`
* **Features:**
  * Dynamic rendering of adjacency matrix, adjacency list, and edge list representation formats.
  * Interactive drawing canvas for custom node placement.
  * Real-time transformation and cross-highlighting between visual and text representations.
  * Step-by-step execution player to visualize algorithms (BFS, DFS, MST).
* **Parameters & Options:**
  * Graph structure mode (Directed/Undirected, Weighted/Unweighted).
  * 0-indexing vs. 1-indexing (dummy node inject).
  * Predefined graph layouts (Complete, Cycle, Tree, DAG, Star, Wheel, Random).
* **Input Formats:** Mouse/pointer interactions, space-separated edge list, adjacency matrix text, adjacency list text.
* **Output Formats:** SVG graphical representation, side-by-side textual panel representations (matrix, list, edges).
* **Output Quality:** High-quality educational animation, no file exports.
* **UX Patterns:** Real-time multi-view dashboard syncing, media-player traversal controls.
* **SEO Copy Angles:** Adjacency list vs. matrix, graph algorithms visualization, draw graphs online.

### Competitor 4: MiniWebtool Adjacency Matrix Calculator
* **URL:** `https://miniwebtool.com/adjacency-matrix-calculator/`
* **Features:**
  * Converts graphs between adjacency matrices, edge lists, and adjacency lists.
  * Analyzes structural properties (density, connected components).
  * Calculates vertex degree sequences (in-degrees/out-degrees for directed).
  * Computes walk-count matrices by raising adjacency matrix to powers ($A^2, A^3$).
  * Renders interactive SVG graph.
* **Parameters & Options:**
  * Input Type selector.
  * Vertex Labels override (default alphabetic sequence A, B, C...).
  * Direction mode (Auto-detect, Undirected, Directed).
* **Input Formats:** Adjacency matrix, edge list, adjacency list.
* **Output Formats:** Adjacency matrix, list, edges, degrees, density, connected components, matrix powers, SVG graph.
* **Output Quality:** Accurate math, simple SVG graph layout.
* **UX Patterns:** Tabbed navigation, automatic format/direction detection.
* **SEO Copy Angles:** Degree sequence calculation, path counts using matrix powers, graph density.

### Competitor 5: Scientific Calculator Online
* **URL:** `https://scientificcalculatoronline.io/adjacency-matrix-calculator/`
* **Features:**
  * Bi-directional synchronization: editing visual graph, edge list, or matrix immediately updates all views.
  * Interactive canvas for node plotting and link dragging.
  * Structural analytics (density, path connectivity, cycle detection).
* **Parameters & Options:**
  * Relationship directionality & values toggles.
  * Active Input Interface selector tabs.
* **Input Formats:** Interactive canvas, edge list notation, multi-dimensional array JSON/syntax.
* **Output Formats:** Dynamic graph visualization, adjacency matrix table, 2D array code, edge list text, analytical stats cards.
* **Output Quality:** Highly reactive and responsive web rendering.
* **UX Patterns:** Drag-and-drop connections, bi-directional syncing, instant error messaging.
* **SEO Copy Angles:** Matrix multiplication path finding, graph theory foundations.

---

## 2. Gaps & Fit-to-Model Filter

| Feature Gap | Competitors | In-Model? | Rationale |
| --- | --- | --- | --- |
| **Interactive Canvas (Graph Drawing & Physics)** | CS Academy, GraphOnline, VisuAlgo, Scientific Calculator | **NO** | Out-of-scope for gizza's text form input/output architecture; requires a complex UI framework. |
| **Step-by-step algorithm playback** | VisuAlgo | **NO** | Out-of-scope; gizza tools are direct, one-shot input-to-output utility transforms. |
| **Adjacency List (`list`) as Input** | MiniWebtool, VisuAlgo | **YES** | Easy to parse adjacency lists (e.g. `A: B C`) in Rust WASM. |
| **Incidence Matrix (`incidence`) as Input** | GraphOnline | **YES** | Easy to parse incidence matrices (where vertices are rows, columns are edges). |
| **Auto-Detect Input Format (`from="auto"`)** | MiniWebtool | **YES** | Checking text structure allows auto-detecting format without requiring the user to explicitly select it. |
| **Graph Statistics/Metrics Output (`to="stats"`)** | MiniWebtool, Scientific Calculator | **YES** | High mathematical value: calculate vertices, edges, density, connectivity, degrees, and cycles. |
| **Matrix Powers walk-count calculation (`power` parameter & `to="power"`)** | MiniWebtool | **YES** | Multiplies the adjacency matrix by itself $k$ times to find walk counts of length $k$. |
| **Robust Delimiters (arrows/colons) in Edge List** | MiniWebtool, Scientific Calculator | **YES** | Parses arrow syntaxes like `A -> B : 5.5` or `A -- B` robustly. |

---

## 3. Selected Improvements

We will implement all in-model gaps:
1. **New `from` options:**
   - `"auto"` (default): Auto-detect input as edges, adjacency matrix, adjacency list, or incidence matrix.
   - `"list"`: Support parsing adjacency lists (e.g., `A: B C` or `A: B(3) C(1.5)` when weighted).
   - `"incidence"`: Support parsing incidence matrices (columns represent edges e1, e2, ...).
2. **New `to` options:**
   - `"stats"`: Print analytical graph statistics (number of nodes, edges, density, degree sequence, connectivity, cycle detection).
   - `"power"`: Raise the adjacency matrix to a user-defined power $k$ (walk counts of length $k$).
3. **New parameters:**
   - `power` (Integer): The power $k$ to raise the matrix to when using `to="power"`. Defaults to 2 (range 1..=10).
4. **Improved edge parsing:**
   - Support arrows/separators in edges: `A -> B`, `A -- B`, `A - B`, `A: B` and weights like `A B : 3.5` or `A B 3.5`.
