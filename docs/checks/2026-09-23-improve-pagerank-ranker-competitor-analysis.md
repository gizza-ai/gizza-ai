# pagerank-ranker — competitor analysis (2026-09-23)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are
paraphrased observations of publicly documented behaviour — no competitor copy, branding,
or trademark is reproduced here or in the tool.

## Scope / dup check

`ls blocks/ | grep -iE 'rank|graph|network'` surfaced three near neighbours; none of them
computes PageRank, so this is a new tool, not a semantic duplicate:

| Existing block | Why it is not a duplicate |
| --- | --- |
| `graph-algorithms` | BFS, DFS, Dijkstra shortest path, topological sort, cycle detection on an edge list. Traversal/pathfinding, no centrality or ranking. |
| `textrank-summarize` | Uses a PageRank-style power iteration internally, but over a *sentence-similarity* graph, and its input/output are prose, not a graph. |
| `percentile-rank-calculator`, `frequent-contacts-ranker` | Rank numeric samples / message counts. No graph structure. |

Decision: **build**. Edge-list parsing deliberately mirrors `graph-algorithms` so the two
graph tools accept the same paste (family invariant).

## Competitors reviewed

1. **MetricGate — PageRank Algorithm Calculator** (`metricgate.com/docs/pagerank/`) — the
   closest web-tool analogue: tabular edge list with an optional weight column.
2. **AgentCalc — PageRank Calculator** (`agentcalc.com/pagerank-calculator.html`) — a
   fixed 5×5 adjacency-matrix form with a damping field.
3. **NetworkX `pagerank`** (`networkx.org`, v3.6) — not a web tool but the de-facto
   reference API; its parameter set is what technical users expect to find.
   (Cross-checked against graph-tool's and Memgraph's `pagerank` signatures, which agree
   on damping / max-iterations / tolerance / weight.)

## Feature matrix

| Capability | MetricGate | AgentCalc | NetworkX | Ours | Verdict |
| --- | --- | --- | --- | --- | --- |
| Edge-list input (source, target) | ✅ | ✖ | ✅ | ✅ | in-model — built |
| Optional weight column | ✅ | ✖ (matrix values) | ✅ (`weight`) | ✅ `weighted` | in-model — built |
| Adjacency-matrix input | ✖ | ✅ (only mode) | ✖ | ✅ `input_format = matrix` (+ `auto`) | in-model — built |
| Damping factor, default 0.85 | ✅ | ✅ | ✅ `alpha` | ✅ `damping` (slider) | in-model — built |
| Max iterations | implied (100) | fixed 100 | ✅ `max_iter=100` | ✅ `max_iter` | in-model — built |
| Convergence tolerance | fixed 1e-10 | ✖ | ✅ `tol=1e-6` | ✅ `tolerance` | in-model — built |
| Iterations-to-converge reported | ✅ | ✖ | ✖ (raises on failure) | ✅ + explicit non-convergence warning | in-model — built |
| Percentage share of total mass | ✅ | ✅ | ✖ | ✅ `share` column | in-model — built |
| In-degree / out-degree columns | ✅ | ✖ | ✖ | ✅ (+ weighted in/out when `weighted`) | in-model — built |
| Personalization vector | ✖ | ✖ | ✅ `personalization` | ✅ `personalization` | in-model — built |
| Dangling-node policy | implicit | implicit | ✅ `dangling` | ✅ `dangling = redistribute \| self-loop \| drop` | in-model — built |
| Undirected graphs | ✖ | ✖ | ✅ (via `Graph`) | ✅ `directed = false` | in-model — built |
| Load-example preset | ✅ | ✖ | n/a | ✅ five `[[example]]` chips | in-model — built |
| Top-N truncation | ✖ | ✖ | ✖ | ✅ `top` | differentiator — built |
| Machine-readable output | CSV/Excel export | copy-to-clipboard | dict | ✅ `format = text \| json \| csv` | differentiator — built |
| Bar chart + convergence plot | ✅ | ✖ | ✖ | ✖ | **out-of-model** — the shared page generator renders text/media outputs only; a per-tool chart renderer would be a slug-specific hack (banned by the platform-over-per-tool-hacks rule). The CSV output feeds any spreadsheet. |
| CSV/Excel file upload | ✅ | ✖ | ✖ | ✖ (paste instead) | **out-of-model** — pure blocks take text params, not file handles; pasting a CSV column pair is equivalent. |
| Embed / share widget, social buttons | ✖ | ✅ | ✖ | ✖ | **out-of-model / declined** — no branding or embed surface in this repo; the page's `?param=` deep links already share a populated run. |
| Arcade practice mini-game | ✖ | ✅ | ✖ | ✖ | **considered, rejected** — off-mission for a calculator. |
| Fixed 5-node ceiling | ✖ | ✅ | ✖ | ✖ | we cap at 20 000 edges / 5 000 nodes instead, stated on the page. |

## Decisions taken into the descriptor

- **Damping defaults to 0.85** everywhere in the field; exposed as a slider (0–0.999,
  step 0.01) because both web competitors make it the one knob users actually turn.
- **Convergence is reported, never hidden.** MetricGate surfaces the iteration count;
  NetworkX raises `PowerIterationFailedConvergence`. We print the iteration count and the
  final L1 delta, and say plainly when `max_iter` was hit before `tolerance` — a partial
  result is labelled, not passed off as converged.
- **Dangling nodes are an explicit choice.** Every competitor buries this. `redistribute`
  (NetworkX's default, mass spread over the jump distribution) is our default; `self-loop`
  and `drop` (the classic leaking variant, where scores sum to < 1) are selectable, because
  textbook worked examples differ on which one they use and users compare against those.
- **Matrix orientation is documented**: `matrix[i][j] > 0` means i → j (row = source).
  AgentCalc uses the transposed convention, so the page states ours explicitly to stop
  silent mismatches; `directed = false` symmetrises either way.
- **Auto format detection** chooses matrix only when every line holds the same number of
  numeric tokens *and* that count equals the line count; anything else parses as an edge
  list. `input_format` forces either.

## Not copied

No competitor wording, examples, styling, or asset was reproduced. The worked examples on
our page are original (a 4-page link graph and a 5-node citation graph), and the numbers in
them are produced by our own implementation and asserted in the test suite.
