# Next Steps

Short handoff note — captures what's still to do after the Plan B MVP
merge. `FUTURE.md` is the long-term catalogue; this file is "what I'd
actually pick up next session."

## Resume here (2026-05-01)

**Just shipped this session:**
- gizza-ai #18 — `web-fetch` + `calculator` skills.
- gizza-ai #20 — `dispatch_skills.rs` LLM-free integration test pattern (closes #19).
- solobase #37 — wasm32 static block registration fix. linkme's `distributed_slice` doesn't run on wasm32, so `solobase-core/src/builder.rs` now explicitly registers the six middleware blocks (cors/inspector/readonly-guard/router/security-headers/web) under `#[cfg(target_arch = "wasm32")]`. **Required for any wasm32 consumer of `SolobaseBuilder`.** gizza-ai's deploy uses `solobase-pin.txt` to pin to a SHA that includes this fix.
- gizza-ai #21 — ffmpeg sub-project 1: `gizza-ai/ffmpeg-runtime` service block + `BrowserFfmpegService` + JS bridge at `js/ffmpeg.js` (lazy-loads `@ffmpeg/ffmpeg` from jsdelivr).
- gizza-ai #22 — ffmpeg sub-project 2: `gizza-ai/ffmpeg` skill (ffprobe scope), two-hop `call_block` (network → bytes → ffmpeg-runtime → log).

**Top of the queue, ordered by ROI:**

1. **Markdown rendering** (Plan E) — afternoon, user-visible polish, no upstream deps. `marked.js` already loaded via the chat plumbing; just route assistant content through it in `gizza-app.js` instead of plain text. Highest user-visible improvement per hour.
2. **ffmpeg sub-project 4** (file preview / inline media) — couples naturally with markdown rendering (could ship as a single PR if the rendering approach handles `<img>`/`<video>` for `data:` URLs). Unlocks ffmpeg sub-project 5+ (resize/transcode/crop/trim ops with visible output).
3. **Conversation persistence** (Plan E, prereq for `search-messages`). **Note:** the previous note here was wrong — `suppers-ai/messages` is registered (`config.rs:187`) but **nothing in gizza-ai writes to it.** Conversation history lives in-memory per request (the client passes prior messages in the agent request body). Refreshing the page loses everything except the selected model in localStorage. Building this is closer to "from zero" than "wire up an existing flow."
4. **Model picker UI** (Plan E) — afternoon, independent.
5. **Code syntax highlighting** — after markdown ships.

**Operational gotchas that bit us this session:**
- `solobase` on `$PATH` is the wrong binary (Go-based). Use the full path `/home/joris/Programs/suppers-ai/workspace/solobase/target/release/solobase`.
- `~/.cargo/bin/wafer` can be stale relative to `wafer-run/main`. Symptom: `wafer build` fails with "function type mismatch for import wafer::__wafer_host_call_block". Fix: `cd wafer-run && cargo install --path crates/wafer-cli --force`.
- Always `--no-track` when creating a new branch: `git worktree add --no-track -b NEW ../path origin/main`. Plain `git checkout -b NEW origin/main` silently sets upstream to `origin/main` and a later `git push` pushes to main.
- The Playwright e2e (`tests/e2e_smoke.spec.ts`) is gated on headed Chromium (WebGPU) and a 1.2 GB WebLLM download; it's a manual smoke, not CI. The deterministic CI gate is `cargo test --test dispatch_skills` (see `tests/README.md`).

---

## Recommended order

### 1. Deploy (Plan C — DONE, two operator items remain)

- [x] GitHub Actions workflow: run `solobase build` on push to `main`, deploy `pkg/` to `gh-pages` branch.
- [x] Add `CNAME` with `gizza.ai` to the deploy artifact.
- [ ] Configure GitHub Pages for the repo (Settings → Pages → source: gh-pages). _Operator action._
- [ ] DNS: CNAME `gizza.ai` to `gizza-ai.github.io` (and www. if desired). _Operator action._
- [x] Switch `.cargo/config.toml` patches off by default — moved to `.cargo/config.toml.example` and gitignored.
- [x] Bump `Cargo.toml` deps to pin specific merged commit SHAs (via `solobase-pin.txt` for the deploy job).
- [ ] Optional: auto-reload on `navigator.serviceWorker.oncontrollerchange` in `loader.js` — zero-reload updates. _Moved to a follow-up in solobase since `loader.js` lives in `solobase-browser`._

### 2. Full v1 skill set (Plan D)

- [x] `gizza-ai/web-fetch` — fetch a URL via `call_block("wafer-run/network", …)`.
- [x] `gizza-ai/calculator` — eval arithmetic via `meval`. Zero deps.
- [~] `gizza-ai/ffmpeg` — flagship. Decomposed into sub-projects (see specs). Status:
  - [x] **Sub-project 1** — ffmpeg-runtime invocation primitive (PR #21). Native `FfmpegBlock` registered as `gizza-ai/ffmpeg-runtime`, JS bridge at `js/ffmpeg.js` lazy-loads `@ffmpeg/ffmpeg` from jsdelivr.
  - [x] **Sub-project 2** — ffprobe-scope skill (PR #22). Two-hop `call_block` (network → bytes → ffmpeg-runtime → log). Tool surface: `{url}` only; returns `{url, info}` JSON.
  - [ ] **Sub-project 3** — file drag-drop UI (deferred — needs design pass).
  - [ ] **Sub-project 4** — file preview / inline `<img>`/`<video>` rendering (deferred — couples with markdown rendering, see Plan E).
  - [ ] **Sub-project 5** — resize/transcode/crop/trim skill ops (blocked on 3+4 because binary output has nowhere usable to land today).
- [ ] `gizza-ai/search-messages` — calls `suppers-ai/messages` via `ctx.call_block` to search past conversation. **Blocked**: chat history isn't persisted to `suppers-ai/messages` today. Build conversation persistence (Plan E) first.

### 3. UX polish (Plan E)

- [ ] **Markdown rendering** — `marked.js` loaded via `ai-bridge.js` (verify); render assistant content through it in `gizza-app.js` instead of plain text. Naturally pairs with ffmpeg sub-project 4 (data: URLs render as `<img>` / `<video>`).
- [ ] **Conversation persistence** — write user/assistant turns to `suppers-ai/messages`; load prior entries on page init. **Note**: starting from zero — the previous note here ("messages already writes to OPFS; UI just doesn't load") was incorrect. `suppers-ai/messages` is registered but unused by gizza-ai today.
- [ ] **Code syntax highlighting** — add `highlight.js` via CDN after markdown lands.
- [ ] **Model picker UI** — replace the hardcoded Qwen2.5-1.5B with a selectable list. Pull from `window.gizzaAI.getAvailableModels()`. Show tool-support badges per model.

## Smaller paper-cuts

- [ ] `security-headers` block reads CSP from flow-step config, not from block_configs. Filed in Plan B commit message (`d83d657`). The clean fix is upstream: have the block also consult `block_configs`, or expose `SolobaseBuilder::csp(...)`.
- [ ] Error taxonomy for `AssetLoadError` could split `LoaderNotConfigured` from `UnknownLoader` (noted in Plan A code-quality review) and `Bridge(String)` from `Unknown(String)` (noted in Plan A Task 5 review). Low urgency.
- [x] `ExternalAsset::timeout_ms` field — shipped as wafer-run #29.
- [ ] Dev server (`python3 -m http.server`) doesn't set `Cache-Control: no-cache` on `sw.js`. Fine for local dev but worth a `_headers` file for GH Pages.
- [ ] Make Playwright e2e suite reliable in CI (model caching across runs via persistent context, OR an LLM-free dispatch path). Tracked as a future item; the current `dispatch_skills.rs` covers the bulk of regressions cheaply, so this is low priority.

## References

- Original design: `docs/superpowers/specs/2026-04-18-gizza-ai-design.md` (workspace sibling)
- Plan A (upstream enablers, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-a-upstream.md`
- Plan B (MVP, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-b-mvp.md`
- Plan C campaign (NEXT.md execution): `docs/superpowers/plans/2026-04-30-gizza-ai-next-md-implementation.md`
- web-fetch: spec `2026-05-01-gizza-ai-web-fetch-design.md`, plan `2026-05-01-gizza-ai-web-fetch.md`
- LLM-free dispatch test: spec `2026-05-01-gizza-ai-llm-free-dispatch-test-design.md`, plan `2026-05-01-gizza-ai-llm-free-dispatch-test.md`
- ffmpeg sub-project 1 (runtime): spec `2026-05-01-gizza-ai-ffmpeg-invocation-primitive-design.md`, plan `2026-05-01-gizza-ai-ffmpeg-invocation-primitive.md`
- ffmpeg sub-project 2 (ffprobe skill): spec `2026-05-01-gizza-ai-ffmpeg-skill-ffprobe-design.md`, plan `2026-05-01-gizza-ai-ffmpeg-skill-ffprobe.md`
- Deferred features catalogue: `FUTURE.md`
