## About this tool

PageRank scores every node in a graph by how much of a random surfer's time it ends up holding. The surfer follows a link with probability `damping` (0.85 by default) and otherwise teleports to a random node, so a node gains authority both from *how many* links point at it and from *how important those linkers are*. It is the standard way to turn "who points at whom" into a single ranked list.

Paste any directed or undirected graph — internal site links, citations, package dependencies, org referrals, Slack mentions, a follow graph — as an edge list or as a square adjacency matrix. The tool runs power iteration locally in Rust/WebAssembly, so nothing is uploaded, and reports the ranked nodes with their score, their share of total PageRank mass, in/out degree, how many iterations it took to converge, and which nodes are dangling.

Accepted edge-list lines, one per line (`#` comments and blank lines are ignored):

```text
a -> b        an edge a → b
a - b         same thing; `a b` and `a,b` also work
a -> b : 3    a weighted edge (used when "Use weights" is on)
a             an isolated node, ranked with no links
```

For a matrix, `matrix[i][j] > 0` means **i → j** — the row is the source. An optional header row and/or a leading label column names the nodes; without labels they are numbered `1`…`n`.

## Worked example

A four-page site where the docs and the API reference link to each other:

```text
home -> docs
home -> pricing
docs -> api
api -> docs
pricing -> home
```

With the defaults (directed, unweighted, damping 0.85) the output is:

```text
PageRank — 4 nodes, 5 edges (directed, unweighted)
Source: edge list · damping 0.85 · tolerance 0.000001 · max 100 iterations · dangling: redistribute
Converged after 26 iterations (final change 8.922e-7)

Rank  Node     PageRank    Share    In   Out
   1  docs     0.416340   41.63%     2     1
   2  api      0.391389   39.14%     1     1
   3  home     0.108611   10.86%     1     2
   4  pricing  0.083660    8.37%     1     1

Total PageRank mass: 1.000000
```

`docs` and `api` win because they trade links with each other and keep recycling the authority between them, while `home` gives away half of its own score to `pricing`. Note that `home` outranks `pricing` even though both have one inbound link — the link into `home` comes from a page that received a share of the `docs`/`api` loop.

Switch the output format to **JSON** or **CSV** to pipe the same ranking into a spreadsheet or a script; the JSON payload adds the weighted in/out totals and the dangling flag per node.

## Choosing the controls

- **Damping** is the one knob people actually turn. 0.85 is the classic value. Lower it (0.5) to favour nodes with many *direct* inbound links and flatten the long chains; raise it (0.95) to let authority travel further along paths, at the cost of slower convergence.
- **Directed** off symmetrises every edge, which turns PageRank into a degree-and-clustering measure for undirected networks (co-authorship, friendships, co-occurrence).
- **Use weights** reads the trailing number on an edge line (or the matrix cell value) as the link strength. A node with out-weights 9 and 1 sends 90% of its score down the first link. With weights off, every non-zero edge counts as 1.
- **Dangling nodes** — nodes with no outgoing links — have to be handled explicitly, and different textbooks pick differently. `redistribute` spreads their mass through the teleport distribution and keeps the total at 1 (this matches the usual library default); `self-loop` lets sinks hoard their own score; `drop` leaks the mass, so scores sum to less than 1, which is the original "leaky" formulation.
- **Personalization weights** bias the teleport target instead of jumping uniformly. `home:0.7, docs:0.3` means every random jump lands on one of those two pages, which produces a "seen from here" ranking (topic-sensitive or rooted PageRank) rather than a global one.
- **Tolerance** and **max iterations** end the loop. Iteration stops when the total absolute score change across all nodes drops below the tolerance; if the cap is hit first, the result is labelled `NOT CONVERGED` rather than presented as final.
- **Top N** trims the printed table only. Scores, shares, and the total mass are always computed over the whole graph.

## Limits and edge cases

- Up to **20,000 edges** and **5,000 nodes** per run. Parallel edges between the same pair are merged and their weights summed, so `a -> b : 2` plus `a -> b : 3` counts as one edge of weight 5.
- Damping must be in `[0, 0.999]`. At exactly 1.0 the iteration has no teleport term and need not converge at all, so it is rejected; `damping = 0` returns the uniform distribution, which is a useful sanity check.
- Edge weights and matrix cells must be zero or positive. Negative values are rejected with the line number rather than silently clamped — PageRank is not defined for them.
- Auto-detection calls a paste a matrix only when it is unambiguous: two or more rows, every token numeric, every row the same length, and that length equal to the row count. Anything else (including a labelled matrix) parses as an edge list, so set **Input format** to `Adjacency matrix` explicitly when your matrix has labels.
- Self-loops (`a -> a`) are kept and boost the node, as in the standard formulation. Isolated nodes still receive their teleport share.
- With `dangling = drop`, the reported total mass is deliberately below 1; the ranking order is unaffected, only the absolute scores and shares.
- Ties are broken by first appearance in the input, so re-running the same paste always gives the same order.
- Scores are printed to the chosen number of decimals (0–12); the underlying computation is always double precision.

## FAQ

<details>
<summary>What does the PageRank score actually mean?</summary>

It is a probability: the long-run fraction of time a random surfer spends on that node. That is why the scores sum to 1 (except under `drop`), and why the **Share** column — the score as a percentage — is usually the easier number to read. A score of 0.42 means the surfer is on that node 42% of the time. Scores are only comparable *within one graph*: adding nodes dilutes every score, so compare ranks and shares, not raw values across different runs.

</details>

<details>
<summary>Why is damping 0.85, and what happens if I change it?</summary>

0.85 is the value from the original formulation and has stayed the field default. It means the surfer follows a link 85% of the time and teleports 15% of the time. The teleport term is what makes the answer unique and the iteration converge; without it, a graph with a sink cluster would drain all the score into that cluster. Lower damping converges faster and produces a flatter ranking closer to raw inbound-link counts; higher damping lets authority propagate over longer paths but needs more iterations to settle.

</details>

<details>
<summary>What is a dangling node and why does the policy matter?</summary>

A dangling node has no outgoing links — a PDF, a leaf page, a paper that cites nothing. During iteration it receives score but has nowhere to send it, so the algorithm has to decide what happens to that mass. `redistribute` hands it to the teleport distribution (everyone gets a share, total stays 1). `self-loop` gives it straight back to the node, which makes sinks rank highly — correct if a sink really is a destination. `drop` throws it away, which is the original leaky variant where the scores sum to less than 1. The report always lists which nodes were dangling so you can tell whether the choice matters for your graph.

</details>

<details>
<summary>How do I rank pages relative to one starting point?</summary>

Use the personalization weights. Entering `home:1` sends every random jump back to `home`, so the ranking answers "what is important as seen from the home page" instead of "what is important overall". Nodes that `home` cannot reach end up with a score near zero. You can spread the bias over several seeds, e.g. `home:0.7, docs:0.3`; the weights are normalised for you, so `7, 3` and `0.7, 0.3` behave identically. Every named node must exist in the graph, otherwise the run reports the typo.

</details>

<details>
<summary>Can I use an adjacency matrix, and which direction is a row?</summary>

Yes. Rows are sources: `matrix[i][j] > 0` means an edge from node i to node j. A plain square block of numbers auto-detects as a matrix; add a header row of names, a leading label column, or both to name the nodes, and set **Input format** to `Adjacency matrix` so a labelled block is not mistaken for an edge list. If your matrix uses the opposite convention, transpose it before pasting — or turn **Directed** off, which symmetrises the matrix and makes the orientation irrelevant.

</details>

<details>
<summary>What does "NOT CONVERGED" mean and how do I fix it?</summary>

It means power iteration hit the max-iteration cap while the total score change was still above your tolerance, so the numbers shown are the last iterate rather than the fixed point. The message prints the final change so you can see how close it got. Raise **Max iterations**, loosen **Tolerance**, or lower **Damping** — high damping on a large sparse graph is the usual cause. The result is never silently presented as final.

</details>

<details>
<summary>Is my graph uploaded anywhere?</summary>

No. The page runs the same compiled WebAssembly module the command line uses, entirely inside your browser tab. Nothing is sent to a server, so pasting an internal link graph, a private dependency map, or a citation list is safe. The sharable link this page can build encodes your inputs in the URL itself — which is convenient, but means the link contains your data, so treat a shared URL the same way you would treat the paste.

</details>
