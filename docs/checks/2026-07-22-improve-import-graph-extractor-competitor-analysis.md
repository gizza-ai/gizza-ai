# Competitor analysis — import-graph-extractor (2026-07-22)

Tool function: paste one or more source files, extract the import/require/use
dependency graph, and report who depends on what (plus circular dependencies).

All findings paraphrased from public docs/READMEs — no competitor copy, branding,
or trademarks reproduced.

## Competitors reviewed

### 1. madge (npm) — https://github.com/pahen/madge
- **Features:** builds a module dependency graph, finds circular dependencies,
  lists orphan (unused) modules and leaf modules (no dependencies), and can show
  which files depend on a given module.
- **Module systems / langs:** CommonJS, AMD, ES6, TypeScript, plus CSS
  preprocessors (Sass/Stylus/Less).
- **Outputs:** console list, JSON, Graphviz DOT, and rendered images (SVG/PNG via
  Graphviz).
- **Flags of note:** `--circular`, `--depends <module>`, `--orphans`, `--leaves`,
  `--json`, `--dot`, `--image`, `--exclude <regex>`, `--extensions`.
- **Defaults:** excludes Node core modules and npm packages from the graph unless
  configured otherwise; reads config from `.madgerc` / package.json.

### 2. pydeps — https://github.com/thebjorn/pydeps
- **Features:** Python module dependency visualization; highlights import cycles;
  a "show cycles only" mode reduces the graph to just the cyclic nodes/edges.
- **How it finds imports:** reads import opcodes from compiled bytecode.
- **Outputs:** SVG/PNG images (needs Graphviz installed); usable as a library.

### 3. dependency-cruiser — https://github.com/sverweij/dependency-cruiser
- **Features:** validates + visualizes dependencies against custom rules; detects
  circular dependencies, orphan files, and rule violations.
- **Langs/module systems:** JavaScript, TypeScript, CoffeeScript; ES6, CommonJS,
  AMD; jsx/tsx/vue/svelte.
- **Reporters/outputs:** DOT, JSON, Mermaid, text/err (eslint-style), HTML, CSV.
- **Defaults:** generates a config with sensible architectural rules.

## Table-stakes → where each lands

| Capability | Competitors | Verdict |
| --- | --- | --- |
| Circular dependency detection | madge, pydeps, dep-cruiser | **in-model** → `detect_cycles` (default on), reported in every format |
| Dependents ("who imports X") | madge `--depends` | **in-model** → reverse-edge "Dependents" section |
| Orphan modules (imported by nothing) | madge, dep-cruiser | **in-model** → "Orphans" line in the report |
| Leaf modules (import nothing) | madge | **in-model** → "Leaves" line in the report |
| JSON output | all | **in-model** → `format=json` |
| Graphviz DOT output | madge, dep-cruiser | **in-model** → `format=dot` |
| Mermaid output | dep-cruiser | **in-model** → `format=mermaid` |
| Console list / text report | madge, dep-cruiser | **in-model** → `format=text` (default) |
| Exclude external (npm/stdlib) by default vs include | madge (excluded by default) | **in-model** → `include_external` toggle (we default *on* so pasted-file users see third-party deps; can hide to focus on file↔file edges) |
| Multi-language (JS/TS + Python) | madge (JS/TS), pydeps (Py) | **in-model** → `language` auto-detect + JS/TS, Python, plus Rust `use`/`mod` and Go `import` |
| TypeScript / jsx / tsx / mjs / cjs syntax | madge, dep-cruiser | **in-model** → same JS parser, extensions detected |

## Out-of-model (listed, not built)

- **Rendered SVG/PNG image output** — needs a Graphviz binary / renderer. We emit
  Graphviz **DOT** and **Mermaid** text instead, which the user pastes into any
  renderer. (feasible-but-not-in-browser-wasm for image rasterizing of arbitrary graphs)
- **Whole-project crawl from a filesystem path** — gizza is browser-local with no
  filesystem; input is pasted source, delimited by `=== path ===` headers.
- **Config files (`.madgerc`, package.json, custom rule validation)** — needs a
  project on disk; no filesystem in-browser.
- **Dead-code / unused-export detection** — needs full symbol resolution across a
  whole resolved project (node_modules, tsconfig paths); out of scope for pasted
  snippets.
- **Regex `--exclude` filter** — considered; deferred to keep the schema lean.
  Users can trim pasted files instead. (in-model but rejected for schema simplicity)

## Design decisions

- Files delimited by `=== path ===` (or `--- path ---`) header lines; language is
  auto-detected per file from the path extension, or forced via `language`.
- Internal file↔file edges are resolved strongly for **JavaScript/TypeScript**
  (relative-path resolution incl. `index` files) and **Python** (dotted-module ↔
  file mapping, incl. relative imports and `from pkg import submodule`).
- **Rust** resolves `mod NAME;` declarations to sibling files and classifies
  external crates (first `use` segment); `crate::`/`super::`/`self::` paths are
  reported but not resolved to a specific file (documented limit).
- **Go** extracts `import` specifiers and classifies stdlib vs external
  (dot-in-first-segment heuristic); file↔file resolution is not attempted
  (documented limit).
- Every format (text/json/dot/mermaid) reports the same underlying graph so the
  advertised-values matrix exercises each end to end.
