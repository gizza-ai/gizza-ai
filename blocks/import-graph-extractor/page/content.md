## About this tool

**Import Graph Extractor** turns pasted source files into a dependency graph you can read, diff, or paste into a graph renderer. It is designed for quick architecture checks when you cannot or do not want to run a project-wide crawler: paste a few files, add `=== path/to/file.ext ===` headers, and see which files import each other.

The parser extracts common dependency forms across:

- **JavaScript / TypeScript** — static `import`, side-effect imports, `export ... from`, dynamic `import(...)`, and `require(...)`.
- **Python** — `import`, `from ... import ...`, and relative imports such as `from .helpers import parse`.
- **Rust** — `use`, `pub use`, `extern crate`, and file modules declared with `mod name;`.
- **Go** — single imports and `import ( ... )` blocks.

For JavaScript/TypeScript, relative imports are resolved against pasted file paths, including extensionless and `index.*` imports. For Python, dotted module names are matched to pasted `.py` files and package `__init__.py` files. Rust `mod name;` declarations resolve to sibling module files. Go imports and Rust `crate::` / `super::` paths are listed, but not resolved to arbitrary project files.

### Worked example

Paste this input:

```text
=== src/app.js ===
import { greet } from './util/greet';
import React from 'react';
const fs = require('fs');

=== src/util/greet.js ===
export const greet = () => 'hi';
```

With **Language = auto**, **Output format = text**, **Include external dependencies = on**, and **Detect circular dependencies = on**, the report includes:

- `src/app.js -> src/util/greet.js` as an internal edge.
- `react [package]` and `fs [stdlib]` as external dependencies.
- `src/util/greet.js <- src/app.js` in the dependents section.
- `src/app.js` as an orphan because no pasted file imports it.
- `src/util/greet.js` as a leaf because it imports no pasted file.

### Output formats

- **Text** — readable report with files, edges, external deps, dependents, orphans, leaves, unresolved imports, and cycles.
- **JSON** — structured data for scripts and tests.
- **DOT** — Graphviz `digraph` text for rendering with Graphviz-compatible tools.
- **Mermaid** — `graph LR` text for Mermaid diagrams.

### Limits and edge cases

This is a pasted-source analyzer, not a full package manager or compiler. It does not crawl directories, read `node_modules`, parse `tsconfig` path aliases, run Python import hooks, evaluate conditional imports, or render SVG/PNG images. Aliases and dynamic expressions that are not string literals are intentionally ignored. The input cap is about 2 MB so the browser stays responsive.

For command-line use, the same functionality is available through the generated CLI. Example:

```bash
gizza tool import-graph-extractor input="=== a.js ===
import './b';
=== b.js ===
export const b = 1;" language=auto format=text include_external=true detect_cycles=true
```

## FAQ

<details>
<summary>How do I paste multiple files?</summary>

Put a header line before each file, using either `=== path/to/file.ext ===` or `--- path/to/file.ext ---`. The path becomes the graph node name, and in `auto` language mode the extension (`.js`, `.ts`, `.py`, `.rs`, or `.go`) tells the analyzer which import rules to use for that file.

</details>

<details>
<summary>Can it find circular dependencies?</summary>

Yes. When **Detect circular dependencies** is enabled, the tool computes cycles among pasted files and shows loops such as `pkg/a.py -> pkg/b.py -> pkg/a.py`. External packages are not part of cycle detection because they are not pasted source files.

</details>

<details>
<summary>Why are some imports listed as unresolved?</summary>

An import is unresolved when it looks like a file reference but no matching pasted file was provided, or when the language feature needs project context the tool does not have. For example, `./missing` in JavaScript, `mod helper;` without `helper.rs`, or Rust `use crate::...` paths may appear there.

</details>

<details>
<summary>Does it replace project-wide tools like dependency graph crawlers?</summary>

No. It complements them for quick, local checks on selected files. Full crawlers can read a filesystem, package manager config, TypeScript path aliases, and generated files; this browser tool only analyzes the source text you paste.

</details>
