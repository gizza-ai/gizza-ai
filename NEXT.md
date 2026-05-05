# Next Steps

Short handoff note — captures what's still to do after the Plan B MVP
merge. `FUTURE.md` is the long-term catalogue; this file is "what I'd
actually pick up next session."

## Resume here (2026-05-05 — post config-fix merge)

Sub-project 4 (inline media rendering) merged in PR #26. The envelope wire format `{_for_llm, _for_ui}` is now the documented way for skills to return non-text output to the UI. `gizza-ai/image-fetch` is the first envelope-emitting skill. CI now runs cargo + npm tests on every PR (test.yml workflow added — gizza-ai had no CI test gate before). PR #27 then unblocked the wasm-pack build by handling the `Result<String, JsValue>` bridge signature and bumping `solobase-pin.txt` to `e95697a` (post-solobase #47).

**Top of the queue:**

1. **Operator items below — your action only.** Unblock the live site.
2. **ffmpeg sub-project 5** — resize/transcode/crop/trim. Now unblocked by sub-project 4. Each transcoding op becomes a separate skill that returns `_for_ui.data_url`. Image ops are straightforward; video transcoding can use the `media-src` CSP entry added in sub-project 4. **Caveat:** the wafer-run wasmi runtime currently JSON-encodes `Vec<u8>` as a number array (~6× size inflation), so binary payloads >2-3 MiB OOM the wasm runtime before reaching skill code. Image-fetch worked around this with a Content-Length pre-check; sub-project 5 will need a wafer-run-side fix (base64 transport for `Vec<u8>`) for video sizes.
3. **ffmpeg sub-project 3** (file drag-drop UI) — needs a design pass first.
4. **Conversation persistence** — still blocked on producer-side solobase change.

**Cross-repo news from the 2026-05-03 session:**

- **solobase #47 (browser vector + embedding backend) is MERGED.** Ships
  `BrowserVectorService` (sql.js + FTS5) + `BrowserEmbeddingService`
  (Transformers.js bridge) + `suppers-ai/transformers-embed` block, all
  wired through `SolobaseBuilder::vector_service(...)` /
  `embedding_service(...)`. If gizza-ai wants to bump `solobase-pin.txt`
  to pick up the new vector backend (relevant for `search-messages` and
  any future RAG skill), the SHA is on solobase `main` post-#47. Nothing
  in gizza-ai breaks without bumping — the pin is opt-in.
- **Smoke test infrastructure on solobase is now reliable.** PR #48/#49/
  #51 fixed the SW-registration smoke (lazy WebLLM ESM import,
  waitUntil:`commit`, waitForURL instead of racing goto). If you copy
  any of solobase-web's smoke patterns into gizza-ai's e2e, those are
  the green ones to imitate.
- **A real product bug was caught and fixed in solobase #47**:
  `scripts/build-sql-js-fts5.sh` had been overriding sql.js's full
  `SQLITE_COMPILATION_FLAGS` with just our 4 flags, dropping
  `-DSQLITE_OMIT_LOAD_EXTENSION` (the killer — leaves dlopen stubs as
  null function pointers in wasm and traps with "null function" the
  first time `new SQL.Database()` runs in a Service Worker). The
  rebuilt FTS5 wasm is 761 KB (vs the broken 1.3 MB one). **Lesson for
  any future custom-emcc builds in this repo: never override an
  upstream `*_COMPILATION_FLAGS` macro without preserving the defaults
  — extend the list, don't replace it.**

**Operator items (your action) — still outstanding from Plan C:**

- Configure GitHub Pages for the repo (Settings → Pages → source: `gh-pages`).
- DNS: CNAME `gizza.ai` → `gizza-ai.github.io` (and `www.` if desired).

**Operational gotchas that bit us in past sessions:**

- `solobase` on `$PATH` is the wrong binary (Go-based). Use the full path `/home/joris/Programs/suppers-ai/workspace/solobase/target/release/solobase`.
- `~/.cargo/bin/wafer` can be stale relative to `wafer-run/main`. Symptom: `wafer build` fails with "function type mismatch for import wafer::__wafer_host_call_block". Fix: `cd wafer-run && cargo install --path crates/wafer-cli --force`.
- Always `--no-track` when creating a new branch: `git worktree add --no-track -b NEW ../path origin/main`. Plain `git checkout -b NEW origin/main` silently sets upstream to `origin/main` and a later `git push` pushes to main.
- The Playwright e2e (`tests/e2e_smoke.spec.ts`) is gated on headed Chromium (WebGPU) and a 1.2 GB WebLLM download; it's a manual smoke, not CI. The deterministic CI gate is `cargo test --test dispatch_skills` (see `tests/README.md`).
- **Debugging Service Worker tests in Playwright**: `page.on('console')` only captures the page's console, NOT the SW's. To see SW logs, hook `context.on('serviceworker', sw => sw.on('console', ...))`. The 2026-05-03 solobase smoke debug took several CI cycles to find this — saved the diagnostic snippet in solobase's git history if you need it.
- **Rebuilding sql.js FTS5**: Docker Desktop must be running. Run `bash scripts/build-sql-js-fts5.sh` (in solobase) — takes ~5 min in the `emscripten/emsdk:3.1.74` container. The script now passes the FULL upstream `SQLITE_COMPILATION_FLAGS` plus `-DSQLITE_ENABLE_FTS5`; do NOT shorten that list (see #47 commit `1373975` for why).

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
  - [x] **Sub-project 4** — inline `<img>`/`<video>` rendering (PR #26). Envelope wire format `{_for_llm, _for_ui}` bifurcates LLM history from UI rendering. Demonstrating skill `gizza-ai/image-fetch` proves the path. Renders inside the tool-call row. Sub-project 5 inherits the envelope; image transcoding can ship without further wire changes (video transcoding uses the `media-src` CSP entry added here).
  - [ ] **Sub-project 5** — resize/transcode/crop/trim skill ops (no longer blocked on 4; sub-project 3 still deferred and orthogonal).
- [ ] `gizza-ai/search-messages` — calls `suppers-ai/messages` via `ctx.call_block` to search past conversation. **Blocked**: chat history isn't persisted to `suppers-ai/messages` today, and persistence itself is blocked on a producer-side change (see Plan E).

### 3. UX polish (Plan E)

- [x] **Markdown rendering** — shipped in PR #14. `marked.parse(raw, { breaks: true, gfm: true })` in `gizza-app.js::renderAssistantContent`; CDN load via `<script src="...marked@13.0.0/marked.min.js">` in `src/blocks/ui.rs`. Re-renders on each token; HTML-escape-by-default.
- [ ] **Conversation persistence** — write user/assistant turns to `suppers-ai/messages`; load prior entries on page init. **Blocked on producer-side (solobase) change**: the `suppers-ai/messages` list endpoint requires authentication and gizza-ai runs anonymous. Either an anonymous list path or a server-side helper that synthesizes auth needs to land in solobase first.
- [x] **Code syntax highlighting** — shipped in PR #14. `hljs.highlightElement` over `pre code` blocks inside `renderAssistantContent`; github-dark theme + highlight.min.js loaded via cdn.jsdelivr.net (also added to CSP `style-src`).
- [x] **Model picker UI** — shipped in PR #14. `<select id="model-picker">` populated client-side from WebLLM's `prebuiltAppConfig.model_list` via dynamic import; selection persists in `localStorage` under `gizza.selectedModel`; tool-supporting families (Hermes-2/3, Qwen2.5, Llama-3-Groq, functionary) get a 🔧 marker.

## Smaller paper-cuts

- [ ] `security-headers` block reads CSP from flow-step config, not from block_configs. Filed in Plan B commit message (`d83d657`). The clean fix is upstream: have the block also consult `block_configs`, or expose `SolobaseBuilder::csp(...)`.
- [ ] Error taxonomy for `AssetLoadError` could split `LoaderNotConfigured` from `UnknownLoader` (noted in Plan A code-quality review) and `Bridge(String)` from `Unknown(String)` (noted in Plan A Task 5 review). Low urgency.
- [x] `ExternalAsset::timeout_ms` field — shipped as wafer-run #29.
- [x] Dev server (`python3 -m http.server`) doesn't set `Cache-Control: no-cache` on `sw.js`. Resolved as well as it can be: `static/_headers` ships a `Cache-Control: no-cache` rule for `/sw.js` (PR #15) and `loader.js` registers the SW with `updateViaCache: 'none'`. Note: GitHub Pages doesn't honor `_headers` (Netlify/Cloudflare convention), so the file is dead weight on the current target — the real fix is the client-side `updateViaCache: 'none'`. If we ever switch hosts to Cloudflare/Netlify the file is already in place.
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
