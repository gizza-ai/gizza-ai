# gizza tools

A library of small, single-purpose compute tools — calculators, converters,
text/data utilities, crypto, image and video (ffmpeg-backed), and more — each
implemented once as a [WAFER](https://github.com/wafer-run/wafer-run) WASM
block and exposed identically through three surfaces: a headless CLI, an MCP
server for agents, and a static, SEO-friendly web page. There is a single
source of truth per tool (the block's `info()`/schema); nothing is
reimplemented per surface.

The hosted, browser-based version of these tools — chat UI, model picker, and
more — lives at **[gizza.ai](https://gizza.ai)**. This repository is the
public, MIT-licensed toolkit that powers it: the tool implementations
(`blocks/`), the `gizza` CLI (`cli/`), and the static tool-page generator
(`tools/generator`).

## Layout

- `blocks/<slug>/` — one WASM block per tool (`core/` pure logic, `web/` browser
  bindings, `page/` SEO copy + form metadata, `manifest.json` schema).
- `block-utils/` — shared Rust helpers used across blocks.
- `cli/` — the `gizza` binary: runs any block headlessly via the wasmi runtime.
- `tools/generator/` — renders each block's `page/` into a static, standalone
  HTML page (`pkg/tools/<slug>/`).
- `js/`, `site/` — small runtime JS/CSS shared by the generated tool pages.
- `tests/` — Playwright end-to-end specs for generated tool pages, plus a few
  `node --test` unit suites (`js/*.test.js`).
- `scripts/` — toolchain bootstrap, the tool-hygiene gate, and the
  scaffold-a-new-tool script.

## Build

Bootstrap the toolchain (Rust + wasm targets, `wasm-pack`, Node, ffmpeg):

```bash
scripts/bootstrap-toolchain.sh
```

Build a block's WASM (repeat per tool, or loop over `blocks/*`):

```bash
cd blocks/<slug>
cargo build --target wasm32-wasip1 --release
mkdir -p target
cp target/wasm32-wasip1/release/*.wasm target/block.wasm
```

Build the CLI:

```bash
cargo build --manifest-path cli/Cargo.toml --release   # → cli/target/release/gizza
```

Render generic (unbranded) static tool pages from every block's `page/`
metadata:

```bash
cargo run --manifest-path tools/generator/Cargo.toml -- .   # → pkg/tools/<slug>/
```

Serve them locally:

```bash
python3 -m http.server --directory pkg 8001
```

## CLI: run tools from a terminal or an agent

The `gizza` CLI boots the wasmi runtime, loads a block's `blocks/<slug>/target/block.wasm`
artifact, and dispatches one tool call — so each tool has a single source of
truth (no separate CLI re-implementation). Tool schemas come from each block's
`info()`.

```bash
# run a tool — positional, key=value, or full JSON
gizza tool calculator "2*2"                          # → 4
gizza tool web-fetch url=https://example.com
gizza tool image-resize url=https://…/cat.png width=640 --out thumb.png
gizza tool calculator --json '{"expr":"sqrt(16)"}'

# discover tools
gizza list                  # name + description for every tool
gizza describe calculator   # the tool's JSON-Schema parameters

# machine-readable output for scripts / agents
gizza tool calculator "2*2" --json-out               # full {_for_llm,_for_ui} envelope
```

Exit codes: `0` ok · `1` tool error · `2` usage/bad args · `3` unsupported in the
CLI. Image/video tools shell out to the system `ffmpeg` (install it to use them);
GPU-only tools (e.g. `imagine`) need a browser GPU and so return an
`unsupported_in_cli` error here.

### MCP server

`gizza mcp` serves the same tool set over the Model Context Protocol (stdio,
newline-delimited JSON-RPC) — register it with an MCP client instead of
shelling out, e.g. `claude mcp add gizza -- /path/to/gizza mcp`.

### For agents — `SKILL.md`

`SKILL.md` (repo root) is a small static doc of the `gizza tool …` contract that
points agents at `gizza list` (the live tool set) and `gizza describe <name>`
(a tool's schema). It deliberately does NOT enumerate tools, so it stays tiny
and never drifts as tools are added.

### Embedding in another program

The `gizza-cli` crate also exposes a small library API — `run_tool(name, json)`,
`list_tools()`, `describe_tool(name)` — for Rust programs that want the tools
without shelling out. Full reference: [`cli/README.md`](cli/README.md).

## Tests

```bash
npm install --no-audit --no-fund && npm test   # js/*.test.js unit suites
cargo test --manifest-path cli/Cargo.toml
cargo test --manifest-path block-utils/Cargo.toml
cargo test --manifest-path tools/generator/Cargo.toml

# generated tool-page Playwright specs (after building the pages and a block's web/pkg)
cd tests && npm install && npx playwright test
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to add or improve a tool. All
contributions are made under the MIT license (see [LICENSE](LICENSE)).
