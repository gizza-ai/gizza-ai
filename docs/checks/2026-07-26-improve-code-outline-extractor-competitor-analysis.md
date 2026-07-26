# Code Outline Extractor — Competitor Analysis (2026-07-26)

Research for a browser-local "code outline extractor": paste source code, get a
structured/hierarchical outline of functions, classes, and methods (like a
symbol tree / table-of-contents / VS Code Outline view). Our target
implementation is pure Rust compiled to wasm, running entirely in the browser
with no server, no full language parser, and no LSP — it relies on brace-depth
and indentation heuristics.

All descriptions below are paraphrased from the public tool pages; no competitor
copy or branding is reproduced.

---

## Competitor profiles

### 1. AST Explorer (astexplorer.net)
- **URL:** https://astexplorer.net/
- **What it is:** The canonical web tool for inspecting the abstract syntax tree
  a parser produces from pasted code. Broad reach and the de-facto reference for
  this category.
- **Languages / parsers:** Many, selectable — JavaScript/TypeScript (multiple
  parser backends), JSX, CSS, HTML, GraphQL, Markdown, and additional languages
  through pluggable parsers. Language/parser is a user choice, not auto-detected.
- **Input:** Paste into an editor pane, or drag-and-drop a source file.
- **Output:** Interactive, collapsible AST tree. Moving the cursor in the source
  highlights the matching tree node (two-way source↔tree sync). Node types and
  properties are inspectable. Not a compact "function/class only" outline — it
  shows the full parse tree at every node granularity.
- **UX controls:** Parser picker with per-parser config/settings, live re-parse
  as you edit, save/fork snippets with a shareable URL, and an experimental
  transform pane for prototyping code mods.
- **Defaults:** Opens with a default JS parser and a sample snippet; live parsing on.
- **Limits:** None stated publicly; runs in-browser (parsers bundled/loaded client-side).
- **Fit note:** Powerful but AST-granular and parser-driven — heavier and lower-level
  than a navigation outline.

### 2. Code Structure Viewer (marcelamayr, GitHub, client-side)
- **URL:** https://github.com/marcelamayr/Code-Structure-Viewer
- **What it is:** A client-side tool to load, filter, and navigate code
  structure, aimed at codebase exploration and preparing code for LLM prompts.
- **Languages:** Structure extraction targets C# and VB.NET (`.cs`/`.vb`);
  syntax highlighting is available for other common languages.
- **Input:** Upload a single file (with an optional language hint) or an entire
  folder/directory of files. All processing is local — nothing is uploaded.
- **Output / view modes:** Four rendering modes — Full Code, Readable (strips
  comments and excess whitespace), Signatures Only (extracts class/method/
  property signatures — closest to an outline), and Raw Minified (aggressively
  shortened identifiers). A left-hand file tree with expand/collapse; clicking a
  folder aggregates code from all files beneath it.
- **Filtering:** Per-element checkboxes to include/exclude comments, namespaces,
  classes, interfaces, methods, properties, fields, events, and delegates.
- **UX controls:** Dual search (content search with match highlighting, plus a
  tree/file-name filter); copy-to-clipboard; a built-in GPT-style tokenizer that
  estimates token counts for original vs processed output.
- **Defaults:** Not explicitly stated; opens on the file tree with full code.
- **Limits:** None stated; fully client-side.
- **Fit note:** The single most on-target competitor for "extract signatures +
  filter by symbol kind," though language coverage is narrow.

### 3. AST Visualizer (muningis.lt)
- **URL:** https://muningis.lt/projects/ast-visualizer
- **What it is:** A visual, learning-oriented AST viewer emphasizing a graphical
  overview over raw JSON.
- **Languages:** TypeScript and JavaScript.
- **Input:** Paste or type code into an editor panel; the diagram updates in
  real time.
- **Output:** An interactive, zoomable node-graph tree (variables, functions,
  expressions, statements shown as connected nodes) rather than a JSON dump.
- **UX controls:** Click-and-drag pan, scroll-to-zoom, click a node to inspect
  its details, live re-parse on edit.
- **Defaults:** Live parsing on; sample code on load.
- **Limits / export:** No copy/export documented; no size limits stated.
- **Fit note:** Positions itself for teaching and quick visual overview — the
  "navigation/overview" framing is close to ours, but graph-style not list-style.

### 4. JavaScript AST Explorer (DevToolkit)
- **URL:** https://heysaiyad.github.io/dev-toolkit/tools/ast-explorer/index.html
- **What it is:** A single-purpose JS AST viewer inside a dev-tools collection.
- **Languages:** JavaScript only.
- **Input:** Paste code.
- **Output:** Two views — an interactive collapsible tree and the raw JSON of the
  syntax tree. Function/class definitions appear as part of the full tree.
- **UX controls:** A "Parse" button to run parsing; toggles for Tolerant Parsing,
  Include Comments, and Include Locations (source positions); Expand All /
  Collapse All buttons; a Copy-JSON button.
- **Defaults:** Parse-on-demand via button; toggles default off unless set.
- **Limits:** None stated; JS input only.
- **Fit note:** Good reference for table-stakes toggles (comments, source
  locations) and copy/expand-collapse controls.

### 5. SQL AST Explorer (General SQL Parser)
- **URL:** https://docs.sqlparser.com/demos/ast-explorer/
- **What it is:** A hosted demo that parses SQL and renders its parse tree for
  node-level analysis. SQL-specific, but a strong reference for tree-viewer UX.
- **Languages / dialects:** SQL across 34+ database dialects (Oracle, SQL Server,
  MySQL, PostgreSQL, BigQuery, Snowflake, Hive, Spark SQL, Presto, Redshift, DB2,
  Teradata, etc.), selectable.
- **Input:** Paste SQL statements.
- **Output:** Interactive hierarchical parse-tree; click nodes to inspect
  properties/relationships.
- **UX controls:** Live parse-as-you-type, expand/collapse nodes, search + node-
  type filtering, a statistics panel (node count, tree depth, complexity),
  AST-to-AST comparison of query variants, screenshot capture, and export to
  JSON or XML. Also offers cross-dialect SQL regeneration.
- **Defaults:** Live preview on; a default dialect selected.
- **Limits:** None stated.
- **Fit note:** Richest UX in the set (search, stats, export, screenshot) — useful
  as a feature-ceiling reference even though the domain (SQL) differs.

---

## Table-stakes summary

Params / options that at least one competitor ships:

| Capability | Seen in | Notes for our tool |
|---|---|---|
| Language selection (explicit picker) | AST Explorer, SQL AST Explorer | Table stakes; auto-detect is a nice-to-have. |
| Auto-detect language | (weak; mostly explicit selection) | Rare — most tools make the user pick. |
| Output as hierarchical tree | All five | Core deliverable. |
| Output as JSON | JS AST Explorer, SQL AST Explorer | Common secondary format. |
| Output as Markdown / plain outline | (gap — none ship a clean MD TOC) | Differentiation opportunity. |
| Signatures-only extraction | Code Structure Viewer | Directly on-target for an outline. |
| Filter by symbol kind (class/method/field/etc.) | Code Structure Viewer | Checkbox filters per kind. |
| Include/exclude comments | JS AST Explorer, Code Structure Viewer | Common toggle. |
| Source line numbers / locations | JS AST Explorer ("Include Locations") | Common toggle. |
| Live preview (parse as you type) | AST Explorer, AST Visualizer, SQL AST Explorer | Expected default. |
| Parse-on-button (explicit trigger) | JS AST Explorer | Alternative to live. |
| Expand All / Collapse All | JS AST Explorer, SQL AST Explorer | Standard tree control. |
| Copy to clipboard | Code Structure Viewer, JS AST Explorer | Table stakes. |
| Download / export (JSON/XML) | SQL AST Explorer | Nice-to-have. |
| Search within output | Code Structure Viewer, SQL AST Explorer | Nice-to-have. |
| Paste input | All five | Core. |
| File / folder upload, drag-drop | AST Explorer (drop), Code Structure Viewer (upload) | Optional. |
| Client-side / privacy (no upload) | Code Structure Viewer, AST Explorer, AST Visualizer | Matches our wasm model — worth highlighting. |
| Token count estimate | Code Structure Viewer | Nice-to-have for LLM-prep angle. |
| Tree statistics (node/symbol count, depth) | SQL AST Explorer | Cheap to add. |
| Shareable URL / save snippet | AST Explorer | Out of scope for a static tool. |

**Common defaults observed:** an explicit language picker; a hierarchical tree as
the primary view; live-update rendering; comments excluded or toggleable; a copy
button; sample code preloaded on open.

---

## In-model vs out-of-model decisions

Our tool is pure Rust → wasm, browser-local, with no real parser/AST and no LSP.
It uses brace-depth (for C-family / brace languages) and indentation depth (for
Python-style languages) as heuristics. This bounds what we can honestly ship.

### In-model — feasible with brace-depth + indentation heuristics (should implement)
- **Symbol detection by line-prefix patterns:** match `fn`/`function`/`def`/
  `class`/`struct`/`interface`/`impl`/`method`/`func`/`type`/`enum` etc. via
  per-language keyword regexes on lines that open a block. This yields the
  function/class/method list that is the core deliverable.
- **Hierarchical nesting via depth:** use brace nesting depth (braces) or leading
  indentation (Python/YAML-like) to nest methods under classes and inner
  functions under outer ones. A tree/TOC follows directly.
- **Signature capture:** keep the declaration line (name + parameter list as
  written) since it is available verbatim before the opening brace/colon —
  approximating "Signatures Only" without a parser.
- **Output formats:** render the outline as an indented tree, a Markdown nested
  list / table-of-contents, and a simple JSON array of `{kind, name, line,
  depth, children}`. Markdown TOC is a clear gap in the competitor set.
- **Line numbers / jump anchors:** trivial — we already track line indices.
- **Filter by detected kind:** checkboxes to show/hide functions vs classes vs
  methods, driven by the keyword that matched (heuristic, not semantic).
- **Comment/blank stripping:** line-based removal of `//`, `#`, `/* */`, `--`
  comments before scanning (best-effort, not string-aware).
- **Public/private filter (heuristic only):** approximate via convention —
  leading underscore (Python), `pub`/`export`/access-modifier keyword presence,
  or capitalization (Go). Must be labeled as heuristic.
- **Expand/collapse, copy button, live preview, symbol count / max depth stats,
  language picker with a lightweight auto-detect guess** (from keyword frequency
  and brace-vs-indent style): all cheap and client-side.

### Out-of-model — needs a real parser / AST / LSP (list as not supported / best-effort caveat)
- **Semantically correct symbol boundaries:** braces or colons inside strings,
  comments, char literals, regex literals, template literals, or here-docs will
  fool depth counting. Guaranteed-correct nesting needs a tokenizer/parser.
- **Accurate signature parsing:** splitting parameters, types, generics, default
  values, and return types reliably requires a grammar, not regex.
- **True public/private/protected resolution:** real access semantics (e.g.
  package-private, friend, module visibility) need language rules.
- **Overloads, decorators/annotations attribution, macro-generated symbols,**
  and expression-assigned functions (e.g. arrow functions bound to consts):
  reliable detection needs an AST.
- **Cross-file / project-wide symbol trees, go-to-definition, references, call
  hierarchy:** require an LSP or multi-file index — outside a single-paste tool.
- **Two-way source↔tree cursor sync (à la AST Explorer):** needs real node source
  ranges; our line-anchored jumps are an approximation, not node-precise.
- **Broad multi-language guarantees:** we can cover a curated set of common
  languages heuristically, but cannot promise correctness across 30+ dialects
  the way parser-backed tools (e.g. the SQL explorer's 34+ dialects) do.
- **AST/parse-tree granularity output (every expression/statement node):** by
  design we emit a symbol outline, not a full syntax tree.

### Recommendation
Implement the in-model list as the shipped feature set: language picker (+ best-
effort auto-detect), heuristic symbol tree with nesting, signature line capture,
kind filters, comment stripping, line numbers, and three output formats (tree /
Markdown TOC / JSON) with copy + download + live preview + expand-collapse +
symbol-count stats. Explicitly document the heuristic limits (strings/comments
fooling depth, no semantic visibility, single-file only) so we match table
stakes honestly without over-claiming parser-grade accuracy. The Markdown-TOC
output and privacy-preserving fully-local operation are the clearest
differentiators against the surveyed tools.
