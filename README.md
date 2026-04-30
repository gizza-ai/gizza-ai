# gizza-ai

Browser-local AI chat site with WASM skill blocks. See the design at
`../docs/superpowers/specs/2026-04-18-gizza-ai-design.md` (workspace sibling).

## Build

```bash
solobase build
```

This:
1. Builds every skill block under `blocks/*` via `wafer build` (auto-discovered).
2. Compiles gizza-ai to WASM via `wasm-pack`.
3. Provisions `sql-wasm.wasm` + `sql-wasm-esm.js` from the sibling solobase checkout.
4. Assembles everything into `pkg/`.

## Serve

```bash
solobase serve
```

First visit registers the Service Worker and reloads. After that the SW intercepts all requests and routes them through the WAFER runtime blocks.

## Local development against unmerged wafer-run/solobase changes

Copy `.cargo/config.toml.example` to `.cargo/config.toml` to point the wafer-* crate names at sibling working copies (`../wafer-run/`). The example file is committed; the active config is gitignored so per-developer overrides don't leak into PRs.

## End-to-end test

```bash
# Prerequisites: pkg/ built via 'solobase build', chromium installed.
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
