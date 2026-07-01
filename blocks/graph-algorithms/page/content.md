## What this tool does

Run a classic **graph algorithm** on a graph you describe yourself. Paste an
edge list, choose an algorithm, and get the answer instantly — right in your
browser. Nothing is uploaded to a server; it runs locally, works offline, and
needs no sign-up.

## How to describe your graph

Put **one edge per line**. The separator is flexible — all of these mean an edge
between `a` and `b`:

```
a -> b
a - b
a b
a,b
```

A line with a single label (like `z`) adds an **isolated node**. Blank lines and
lines starting with `#` are ignored, so you can comment your graph.

For a **weighted** graph, turn on **Weighted** and add a number at the end of the
line:

```
a -> b : 3
a -> c 5
b -> c 1
```

Turn on **Directed** to treat each edge as one-way (`a → b` only). Leave it off
for an undirected graph, where every edge goes both ways.

## Algorithms

| Algorithm | What it computes |
| --- | --- |
| **BFS** (breadth-first search) | Visit order exploring the graph level by level from a start node. |
| **DFS** (depth-first search) | Visit order exploring as deep as possible before backtracking. |
| **Shortest path** | The cheapest route from a start node to an end node (Dijkstra when weighted, fewest hops when not). |
| **Topological sort** | A linear ordering of a directed acyclic graph so every edge points forward. Requires a directed graph. |
| **Cycle detection** | Reports whether the graph contains a cycle, and shows an example. |

BFS, DFS, and shortest path need a **Start** node; shortest path also needs an
**End** node.

## Examples

| Edge list | Algorithm | Result |
| --- | --- | --- |
| `a - b`, `a - c`, `b - d` | BFS from `a` | `a → b → c → d` |
| `a - b`, `a - c`, `b - d` | DFS from `a` | `a → b → d → c` |
| `a→b:1`, `a→c:5`, `b→c:1` (directed, weighted) | Shortest path `a`→`c` | `a → b → c`, distance 2 |
| `a→b`, `a→c`, `b→d`, `c→d` (directed) | Topological sort | `a → b → c → d` |
| `a→b`, `b→c`, `c→a` (directed) | Cycle detection | cycle found: `a → b → c → a` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your graph never leaves your device, and the
tool keeps working offline once the page has loaded.

</details>

<details>
<summary>Directed or undirected?</summary>

Use **Directed** for one-way relationships (task
dependencies, web links, follower graphs). Leave it off for symmetric
relationships (roads, friendships). Topological sort only makes sense on a
directed graph.

</details>

<details>
<summary>What does “weighted” do?</summary>

It tells the parser to read a trailing number on
each edge as a distance/cost. Shortest path then uses Dijkstra’s algorithm.
Weights must be non-negative.

</details>

<details>
<summary>Why does shortest path not take the direct edge?</summary>

Dijkstra finds the *cheapest*
route by total weight — a longer chain of small edges can beat one expensive
direct edge.

</details>

<details>
<summary>What if a node can’t be reached?</summary>

BFS/DFS report how many nodes they reached;
shortest path says no path exists when the end node is unreachable from the start.

</details>
