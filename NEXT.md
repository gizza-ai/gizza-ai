# Next Steps

Short handoff note — captures what's still to do after the Plan B MVP
merge. `FUTURE.md` is the long-term catalogue; this file is "what I'd
actually pick up next session."

## Resume here (2026-05-03)

**Stale-handoff catch:** the previous Resume section listed Plan E items
(markdown rendering, model picker UI, syntax highlighting) as next-up,
but PR #14 (`feat(ui): model picker, markdown rendering, syntax
highlighting`, merged 2026-04-30) shipped all three. They're checked off
in Plan E below now.

**Top of the queue, ordered by ROI:**

1. **ffmpeg sub-project 4** (file preview / inline media) — afternoon-sized.
   Render `data:` URLs returned by skills as `<img>`/`<video>` inside the
   assistant bubble. Markdown rendering already handles `<img src="data:...">`
   when the model emits it, so this is partly about the skill side
   (returning image/video bytes in a renderable shape) and partly about
   exposing those in the UI. Unlocks sub-project 5.
2. **Smaller paper-cut: `_headers` for `sw.js`** — quick GH Pages hygiene
   fix; prevents stale SW caching on deploy. See "Smaller paper-cuts"
   below.
3. **ffmpeg sub-project 3** (file drag-drop UI) — needs a design pass
   first. Pairs symmetrically with sub-project 4 (input vs output).
4. **Conversation persistence** — **blocked** on a producer-side
   (solobase) change. The `suppers-ai/messages` list endpoint requires
   authentication and gizza-ai runs anonymous; wiring it up needs either
   an anonymous list path or a server-side helper the agent block can
   call with synthesized auth. See PR #14 commit message for the full
   reasoning. Until that lands, this is a no-go.

**Operator items (your action) — still outstanding from Plan C:**

- Configure GitHub Pages for the repo (Settings → Pages → source: `gh-pages`).
- DNS: CNAME `gizza.ai` → `gizza-ai.github.io` (and `www.` if desired).

**Operational gotchas that bit us in past sessions:**

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
  - [ ] **Sub-project 4** — file preview / inline `<img>`/`<video>` rendering. Markdown rendering already shipped (PR #14), so this is now standalone — wire skill output (`data:` URLs / typed binary) through to the bubble.
  - [ ] **Sub-project 5** — resize/transcode/crop/trim skill ops (blocked on 3+4 because binary output has nowhere usable to land today).
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
