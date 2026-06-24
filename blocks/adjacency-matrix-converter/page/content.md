## What this tool does

Convert a graph between the three ways people usually write one down — an **edge
list**, an **adjacency matrix**, and an **incidence matrix**. Pick the **Input
format** your data is in and the **Output format** you want, set **Directed** and
**Weighted** to match your graph, and the conversion runs instantly. Nothing is
sent to a server — it works locally, offline, and needs no sign-up.

## The three representations

| Representation | What it looks like | Good for |
| --- | --- | --- |
| **Edge list** | One edge per line: `A B` (or `A B 3` when weighted) | Writing a graph by hand, sparse graphs |
| **Adjacency matrix** | A square `n×n` table; cell `(i, j)` is the edge from `i` to `j` | Algorithms, dense graphs, linear algebra |
| **Incidence matrix** | A `vertices × edges` table; one column per edge | Flow problems, graph Laplacian, circuit analysis |

## Input format

- **edges** (default) — one edge per line. Endpoints are separated by a space or
  a comma (`A B` or `A,B`). Add a third token for the weight (`A B 3`) when
  **Weighted** is on. A line with a single token (`D`) declares an isolated
  vertex. Lines starting with `#` are comments and are ignored.
- **adjacency** — a square numeric matrix, one row per line, cells separated by
  spaces or commas. A header row and/or a leading label column are detected
  automatically; without them, vertices are named `V1, V2, …`.

## Output format

- **adjacency** (default) — a labelled square adjacency matrix (tab-separated,
  with a label row and column).
- **incidence** — a vertices × edges matrix, one column per edge (`e1, e2, …`).
  For an **undirected** graph an endpoint cell holds the edge weight (a self-loop
  is `2×weight`). For a **directed** graph the tail is `-weight` and the head is
  `+weight`.
- **edges** — a normalized edge list (handy for cleaning up or de-duplicating a
  hand-written one, or extracting edges out of a matrix).
- **list** — an adjacency list, one line per vertex (`A: B C`); weighted
  neighbours show as `B(3)`.
- **degree** — the diagonal degree matrix `D`, each vertex's (weighted) degree on
  the diagonal.
- **laplacian** — the graph Laplacian `L = D − A` (undirected graphs only),
  used for spectral graph theory and the number of spanning trees.

## Directed and weighted

- **Directed** — when on, `A B` is a one-way edge and does **not** imply `B A`.
  When off (the default) the graph is undirected and the adjacency matrix is
  symmetric.
- **Weighted** — when on, edge weights are read (the 3rd token of an edge line,
  or the value of a matrix cell) and emitted. When off, every present edge is `1`.

## Examples

| Input | From → To | Settings | Output |
| --- | --- | --- | --- |
| `A B` / `B C` / `A C` | edges → adjacency | undirected | a symmetric 3×3 matrix for the triangle |
| `A B` / `B C` | edges → adjacency | directed | upper-triangular (A→B, B→C only) |
| `A B 5` / `B C 2.5` | edges → adjacency | weighted | cells carry `5` and `2.5` |
| a labelled matrix | adjacency → edges | — | the edge list it represents |
| `A B` / `B C` | edges → incidence | undirected | columns `e1, e2`, two `1`s per column |

## FAQ

**Is it free and private?** Yes — your graph never leaves your device, and the
tool keeps working offline once the page has loaded.

**How are matrices laid out?** Tab-separated, with the vertex labels in the first
row and first column so you can paste the result straight into a spreadsheet.

**What sign convention does the directed incidence matrix use?** The edge's tail
(source) is `-1` (or `-weight`) and its head (target) is `+1` (or `+weight`),
matching the standard oriented incidence matrix used for the graph Laplacian.

**Can I round-trip?** Yes — convert edges → adjacency → edges and you get the same
graph back. Isolated vertices survive because they are written as a single-token
line in the edge-list output.
