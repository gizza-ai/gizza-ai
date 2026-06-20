# jq-query — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/jq-query` — run a jq program against a JSON document. Chat +
CLI + page. Pure-Rust `jaq` (a `no_std`+alloc jq implementation).

## Why jaq (and not a real JS engine)

The adjacent backlog item `javascript-runner` was skiplisted because the only
mature pure-Rust JS engine (`boa`) imports `wasi_snapshot_preview1::poll_oneoff`,
which the gizza wafer runtime doesn't provide, so it can't instantiate. `jaq` is
`#![no_std]` (alloc only) with no WASI/host deps, so it instantiates cleanly in
the same runtime — the right kind of embeddable engine for gizza.

## What competitors do

- **jqplay.org / online jq** — paste filter + JSON, see output. Strength: the
  reference playground. Weakness: runs jq **on a server** (your JSON is uploaded).
- **`jq` CLI** — the canonical tool, fully local, but requires installing jq and
  a shell; not callable by an agent or embeddable in chat.
- **JSONPath/lodash online evaluators** — weaker query languages than jq.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust jaq compiled to wasm: the page
   runs in-browser, the CLI runs headless, and it even runs inside the chat
   Service Worker. The JSON never leaves the device.
2. **Real jq, with the standard library.** `map`, `select`, `group_by`,
   `sort_by`, `add`, `unique`, object/array construction, pipes, arithmetic, etc.
   — jaq implements the jq language, not a reduced subset.
3. **Honest multi-output model.** A jq filter can yield zero, one, or many
   values; each is returned separately (`outputs: string[]` in chat/CLI, one per
   line on the page) instead of being forced into a single blob.
4. **Clear, separated errors.** Invalid JSON, jq **parse**, jq **compile**, and
   **runtime** errors are reported distinctly so you know whether to fix the data
   or the filter.
5. **Pretty or compact.** `pretty=true` indents each output value.
6. **Three surfaces + deep-links.** chat tool, CLI, and a shareable page with
   query-param deep-links.

## Honest scope / notes

- Object keys are emitted in **sorted** order (jaq's behavior).
- jaq targets the jq language; a handful of the newest/obscure jq builtins may
  differ — the common filtering/reshaping surface is covered.

## Build note (reusable)

Like `scraper`, jaq's tree pulls `getrandom 0.3`, so the page
(wasm32-unknown-unknown) needs the `wasm_js` backend: `getrandom` `wasm_js`
feature in `web/Cargo.toml` + a scoped `web/.cargo/config.toml` with
`--cfg getrandom_backend="wasm_js"`. The block's wasm32-wasip1 target is fine.

## Tests

7 core unit tests: identity, field access, array iteration (multiple outputs),
`map(select(...))` from the std library, object construction + pipe across an
array, pretty-printing, and four error cases (bad JSON, empty program, parse
error, runtime type error). Plus the block drift-guard schema test. The block
**instantiates** in the wafer runtime (validated by `wafer build`, where boa
failed). CLI + Playwright (filter via fill, array-iteration via deep-link)
verified — see commit.
