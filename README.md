# gizza-ai

Browser-local AI chat site with WASM skill blocks. See the design at
`../docs/superpowers/specs/2026-04-18-gizza-ai-design.md` (workspace sibling).

## Build

```bash
impresspress build
```

This:
1. Builds every skill block under `blocks/*` via `wafer build` (auto-discovered).
2. Compiles gizza-ai to WASM via `wasm-pack`.
3. Provisions `sql-wasm.wasm` + `sql-wasm-esm.js` from the sibling impresspress checkout.
4. Assembles everything into `pkg/`.

## Serve

```bash
impresspress serve
```

First visit registers the Service Worker and reloads. After that the SW intercepts all requests and routes them through the WAFER runtime blocks.

## CLI: run skill tools from a terminal or an agent

The same skill blocks that power the chat are runnable headlessly through the
`gizza` CLI (in `cli/`). It boots the wasmi runtime, loads the *same*
`blocks/*/target/block.wasm` artifacts, and dispatches one tool call — so each
tool has a single source of truth (no separate CLI re-implementation). Tool
schemas come from each block's `info()`, the same source the chat advertises to
the model.

```bash
# build the blocks first (produces blocks/*/target/block.wasm), then the CLI:
impresspress build
cargo build --manifest-path cli/Cargo.toml --release   # → cli/target/release/gizza

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
`imagine` needs a browser GPU and so returns an `unsupported_in_cli` error here.

### For agents — `SKILL.md`

`SKILL.md` (repo root) is a small static doc of the `gizza tool …` contract that
points agents at `gizza list` (the live tool set) and `gizza describe <name>` (a
tool's schema). It deliberately does NOT enumerate tools, so it stays tiny and never
drifts as tools are added.

### Embedding in another program

The `gizza-cli` crate also exposes a small library API — `run_tool(name, json)`,
`list_tools()`, `describe_tool(name)` — for Rust programs that want the tools
without shelling out. Full reference: [`cli/README.md`](cli/README.md).

## Local development against unmerged wafer-run/impresspress changes

Copy `.cargo/config.toml.example` to `.cargo/config.toml` to point the wafer-* crate names at sibling working copies (`../wafer-run/`). The example file is committed; the active config is gitignored so per-developer overrides don't leak into PRs.

## End-to-end test

```bash
# Prerequisites: pkg/ built via 'impresspress build', chromium installed.
cd tests
npm install
npx playwright install chromium   # first time only (~200 MB download)
npm test
```

The smoke test:
1. Loads the page and waits for the chat UI (served by the `gizza-ai/ui` block via SW).
2. Opens settings, clicks "Load model," waits up to 3 minutes for "Ready"
   (Qwen2.5-1.5B, ~1.2 GB first-visit download cached in the browser).
3. Sends "what is the current time in UTC?" — a prompt designed to trigger the
   `gizza-ai/clock` WASM skill.
4. Asserts that `#messages` contains something matching
   `/time|clock|UTC|\d{2}:\d{2}|\d{4}-\d{2}-\d{2}/i`.

Assertions are loose because WebLLM inference is non-deterministic. The test
is a smoke — it verifies end-to-end plumbing, not model correctness.

## Status

Plan B MVP: single clock skill, hardcoded Qwen2.5-1.5B model, public chat.

Plan C will add: ffmpeg + web-fetch + calculator + search-messages skills,
model picker UI, file drag-drop, and deployment to gizza.ai.

See `FUTURE.md` for the full deferred-items catalogue.
