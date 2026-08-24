# rust-module-map — competitor analysis (2026-08-22)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
Everything below is paraphrased from public docs; no competitor copy, branding, or
trademarked wording was reused.

## 0. Duplicate / viability check (done first)

`ls blocks/ | grep -iE 'rust|module|code|tree|ast|mermaid|graph'` surfaced nine plausible
neighbours. Each was opened before deciding to build:

| Existing block | Why it is NOT this tool |
| --- | --- |
| `code-outline-extractor` | Language-agnostic **heuristic brace scanner** over 11 languages (`collect_brace`/`scan_brace` in its `core/src/lib.rs`); emits a symbol tree / Markdown TOC / JSON. It has no Rust parser, no `mod`-declaration semantics, no visibility (`pub(crate)`/`pub(in …)`) reporting, no `impl Trait for Type` grouping, no `#[cfg(test)]` handling, and no graph output. Nearest neighbour, but a different mechanism and a different answer. |
| `ast-diff` | Also Rust + `syn`, but it **compares two** sources structurally. No hierarchy rendering. (It is the proof that `syn` instantiates under wafer — reused here.) |
| `import-graph-extractor` | Cross-**file** import edges / cycles / orphans for JS, Python, Rust, Go. Edges between files, not the intra-crate item hierarchy. |
| `code-metrics-analyzer` | LOC / complexity / maintainability numbers, no hierarchy. |
| `dot-to-mermaid`, `mermaid-from-data`, `json-to-graph` | Generic format converters — they take a graph as input, they do not derive one from Rust source. |
| `directory-tree-view`, `file-tree-generator` | Filesystem paths → ASCII tree. No source parsing. |
| `function-grapher` | Plots `y = f(x)` maths. Unrelated. |

Verdict: **not a duplicate, viable as a pure block.** `syn` is already proven wasm-safe in
this repo (`blocks/ast-diff`), so an exact parse — not a regex/brace heuristic — is available
for both the wafer (`wasm32-wasip1`) and browser (`wasm32-unknown-unknown`) targets.

## 1. Sources inspected

1. **cargo-modules** (regexident/cargo-modules) — README on GitHub. The reference
   implementation for this job: <https://github.com/regexident/cargo-modules/blob/main/README.md>
2. **rust-analyzer-modules** — the library extracted from the above, docs.rs crate page:
   <https://docs.rs/crate/rust-analyzer-modules/latest>
3. **Mermaid flowchart syntax** — official docs, for the graph output format:
   <https://mermaid.js.org/syntax/flowchart.html>
4. **`syn::Item`** — docs.rs, the item taxonomy any Rust module map must cover:
   <https://docs.rs/syn/latest/syn/enum.Item.html>

(`crates.io/crates/rust-analyzer-modules` and `lib.rs/crates/cargo-modules` were also tried;
the first renders client-side and returned no content, the second returned HTTP 403. The
docs.rs mirror above covers the same crate, so the scan still rests on four reachable
sources.)

## 2. What the competitors do

**cargo-modules** is a cargo subcommand with three modes: `structure` (hierarchical tree),
`dependencies` (a Graphviz/DOT graph of internal edges) and `orphans` (source files not
reachable from the module tree). Its structure tree prints one line per item as
`<keyword> <name>: <visibility> <attributes>` under box-drawing branches, e.g. a
`mod tests: pub(crate) #[cfg(test)]` node whose child is `fn it_works: pub(self) #[test]`.
Visibility is spelled out in full (`pub`, `pub(crate)`, `pub(in crate::a::b)`, `pub(self)`)
and colour-coded in a terminal.

Flags that matter for a paste-in-the-browser equivalent:

- `--no-fns`, `--no-traits`, `--no-types` — per-kind filters (all kinds shown by default)
- `--cfg-test` — include `#[cfg(test)]` items (excluded by default)
- `--sort-by name|visibility|kind` plus `--sort-reversed`
- `--max-depth <N>` — truncate the tree
- `--focus-on <path>` — restrict to one module path
- `--acyclic`, `--layout dot|neato|…`, `--no-externs`, `--no-uses` — dependency-mode only

**rust-analyzer-modules** is the same analysis as a library; it is rust-analyzer's HIR
lowering, so it resolves a whole crate from `Cargo.toml` on disk.

**Mermaid** is the graph syntax we render into: `flowchart TD`, `id["label"]` nodes,
`a --> b` edges, shape suffixes (`(…)` rounded, `{{…}}` hexagon, `([…])` stadium,
`[[…]]` subroutine) and `classDef`/`class` for styling. Labels containing quotes or the
reserved word `end` must be escaped — handled in the renderer.

**`syn::Item`** defines the 16 item kinds a complete map must classify: `Mod`, `Struct`,
`Enum`, `Union`, `Trait`, `TraitAlias`, `Type`, `Fn`, `Impl`, `Const`, `Static`, `Macro`,
`Use`, `ExternCrate`, `ForeignMod`, `Verbatim`.

## 3. Table stakes → decision

| Table stake (from the scan) | Decision | Where it lands |
| --- | --- | --- |
| Hierarchical indented tree of the crate's items | **In model** | `format = "tree"` (default), box-drawing branches |
| Full visibility per item (`pub` / `pub(crate)` / `pub(super)` / `pub(in path)` / `pub(self)`) | **In model** | rendered as `: <vis>`, toggled by `show_visibility` |
| `#[cfg(test)]` / `#[test]` marked, and excluded by default | **In model** | `include_tests` (default off), attributes annotated on the node |
| Per-kind filters (fns / traits / types) | **In model** | `show_fns`, `show_traits`, `show_types`, plus `show_impls`, `show_consts` |
| `--max-depth` truncation | **In model** | `max_depth` (0 = unlimited) |
| `--focus-on <module path>` | **In model** | `focus_on`, accepts `crate::a::b` or `a::b` |
| `--sort-by name / visibility / kind` | **In model** | `sort_by` (+ a `source` option = declaration order, the default) |
| Graph output (they emit DOT) | **In model, upgraded** | `format = "mermaid"` — renders inline in Markdown/GitHub without a Graphviz install. The repo already ships `dot-to-mermaid` if DOT is wanted. |
| Machine-readable output | **In model** | `format = "json"` and `format = "paths"` (flat `crate::a::B` list) |
| Nested `mod` blocks **and** `mod foo;` file declarations | **In model** | `mod foo;` renders as an unresolved leaf; paste multiple files with `=== src/foo.rs ===` headers (same convention as `import-graph-extractor`) and they are stitched into the tree |
| `impl` blocks grouped, incl. `impl Trait for Type` | **In model** | `impl` nodes with their methods/assoc items as children |
| Colour-coded visibility in a terminal | **Out of model** | The page renders monospace text; colour would not survive copy-paste or the CLI pipe. Not built. |
| `orphans` (unlinked source files) | **Out of model** | Needs a filesystem walk of the crate directory; this block never touches a disk. Not built. |
| Whole-crate resolution from `Cargo.toml`, `#[path]`, `cfg(feature)` gating | **Out of model** | Requires cargo metadata + name resolution (rust-analyzer's HIR). We parse the text you paste; the multi-file `===` convention is the in-model approximation. Stated as a limit on the page. |
| Inter-module dependency edges / cycles (`dependencies --acyclic`) | **Out of model here** | Already covered by the existing `import-graph-extractor` block; duplicating it would be the near-dup the skiplist exists to prevent. |
| Graphviz `--layout` engines | **Out of model** | Mermaid picks its own layout; no engine choice to expose. |

## 4. UX patterns adopted

- `[[example]]` preset chips (the declarative answer to competitors' README examples):
  a nested-module lib.rs tree, the same crate as a Mermaid graph, and a
  tests-included run — one click each.
- `[input.labels]` friendly names on both `<select>`s (`format`, `sort_by`).
- `multiline = true` on the source field so a pasted `lib.rs` keeps its newlines.
- Placeholders on every text/number field; `max_depth` documented as `0 = unlimited`.
- Every param carries a `.describe()` written for an LLM/CLI caller (values + default +
  example), and the descriptor is drift-guarded by a schema unit test.
