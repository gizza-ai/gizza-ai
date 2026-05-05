# gizza-ai tests

Two test surfaces with very different runtime characteristics.

## `cargo test`

Native Rust integration tests. Run in CI, fast, deterministic, headless.

- `tests/skills_embed.rs` — asserts every skill block under `blocks/*/` ends up embedded in `gizza_ai::skills::SKILLS` after `solobase build`. Catches "I forgot to add the new skill" and "wafer compiled to an empty file."
- `tests/dispatch_skills.rs` — boots a `wafer-run::Wafer` runtime in-process, loads the produced `block.wasm` into the `wasmi` runtime, substitutes a `FakeNetworkBlock` at the network leaf, and verifies cross-block dispatch round-trips correctly. Same wasmi loader and `__wafer_host_call_block` ABI used in the browser SW.

```bash
# Prerequisite: blocks/*/target/block.wasm must exist (built via wafer build).
solobase build       # builds every block under blocks/* + the gizza-ai bundle
                     # equivalent: `wafer build blocks/<name>` per block

cargo test
```

## `npx playwright test` — manual smoke test

Drives the real chat UI (boot → load WebLLM model → send prompt → assert tool call result). Useful as a manual sanity check that the deployed site works end-to-end, but **not** runnable in CI today:

- Requires headed Chromium with WebGPU (set in `playwright.config.ts` as `headless: false`). Linux headless Chromium has no WebGPU.
- Downloads ~1.2 GB of model weights (Qwen2.5-1.5B) on each cold run, because Playwright's per-test browser context wipes IndexedDB. Persistent-context fixture work is tracked separately.

```bash
# Prerequisite: pkg/ built via 'solobase build'.
cd tests
npm install
npx playwright install chromium    # first time only
npx playwright test
```

If you need a deterministic, repeatable check that a skill works end-to-end, prefer `cargo test --test dispatch_skills` over the Playwright suite.

## `npm test` — JS unit tests

Pure-JS tests for `pkg/render.js` (the inline-media render module) using `node:test` + `linkedom`. No browser, no model, deterministic.

```bash
npm install   # one-time, installs linkedom
npm test
```

Runs in CI (`.github/workflows/test.yml`).

## Manual smoke: inline media rendering

After `solobase build && solobase serve`, paste a public image URL into chat (e.g., `https://upload.wikimedia.org/wikipedia/commons/thumb/3/3a/Cat03.jpg/320px-Cat03.jpg`). The model should call `gizza-ai/image-fetch` and a thumbnail should appear inside the tool-call row, capped to 240px tall by `.tool-attachment` CSS. If the thumbnail is missing but the tool-call row succeeded, inspect the SSE stream — `tool_result` events should carry a `for_ui` field with `data_url` and `mime`.

## Manual smoke: image-ops skills (sub-project 5)

After `solobase build && solobase serve`, paste a public small image URL (≤ 4 MiB) and ask the model to do one of:

- `"resize this to 256 wide"` — should call `gizza-ai/image-resize`; the resized thumbnail appears inside the tool-call row.
- `"crop the center 200×200 from this"` — should call `gizza-ai/image-crop`; cropped thumbnail appears.
- `"convert this to JPEG quality 70"` — should call `gizza-ai/image-convert`; new JPEG appears (filename ends `.jpg`).

If the thumbnail does not appear, check the SSE stream from `/b/agent/chat`: `tool_result` events should carry a `for_ui` field with `data_url` and `mime`. If `for_ui` is missing, the skill probably hit one of the size/mime caps — see browser console for the agent's LLM-history text from `_for_llm`.
