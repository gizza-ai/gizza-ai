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
