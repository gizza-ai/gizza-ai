# Next Steps

Short handoff note — captures what's still to do after the Plan B MVP
merge. `FUTURE.md` is the long-term catalogue; this file is "what I'd
actually pick up next session."

## Recommended order

### 1. Deploy (Plan C — smallest, highest ROI)

Get gizza-ai live at `gizza.ai` so it's shareable.

- [x] GitHub Actions workflow: run `solobase build` on push to `main`, deploy `pkg/` to `gh-pages` branch.
- [x] Add `CNAME` with `gizza.ai` to the deploy artifact.
- [ ] Configure GitHub Pages for the repo (Settings → Pages → source: gh-pages). _Operator action after PR-A merges._
- [ ] DNS: CNAME `gizza.ai` to `gizza-ai.github.io` (and www. if desired). _Operator action after PR-A merges._
- [x] Switch `.cargo/config.toml` patches off by default — moved to `.cargo/config.toml.example` and gitignored. Copy back locally when developing against unmerged sibling checkouts.
- [x] Bump `Cargo.toml` deps to pin a specific merged commit SHA (not just `branch = "main"`) for reproducible CI.
- [ ] Optional: auto-reload on `navigator.serviceWorker.oncontrollerchange` in `loader.js` — zero-reload updates. Needs a loop-guard (don't reload if a reload is already in flight). _Moved to PR-C in solobase since `loader.js` lives in `solobase-browser`, not gizza-ai._

**Rough estimate:** a few hours to a day. Self-contained.

### 2. Full v1 skill set (Plan D)

Right now there's only `gizza-ai/clock`. The flagship demo is ffmpeg image manipulation.

- [ ] `gizza-ai/web-fetch` — fetch a URL and return its text body. No external asset loader needed (uses the existing `http_fetch` bridge). Good second skill to prove the pattern beyond clock.
- [ ] `gizza-ai/calculator` — eval simple arithmetic expressions in Rust. Zero deps.
- [ ] `gizza-ai/search-messages` — calls `suppers-ai/messages` via `ctx.call_block` to search past conversation. Tests cross-skill dispatch.
- [ ] `gizza-ai/ffmpeg` — the flagship. First skill to exercise the `externalAssets` declarative loader end-to-end. Needs:
  - Skill block wrapping ffmpeg operations (resize, crop, trim, transcode) as a tool.
  - `ai-bridge.js` registers an `ffmpeg.wasm` loader in `_loaderRegistry`, using `@ffmpeg/ffmpeg` from jsdelivr.
  - File drag-drop in the UI so the user can supply an image/video.
  - File preview in assistant replies (inline `<img>`/`<video>`/download button).

**Rough estimate:** ffmpeg alone is a session or two; the others are an afternoon each.

### 3. UX polish (Plan E)

- [ ] Model picker UI — replace the hardcoded Qwen2.5-1.5B with a selectable list. Pull from `window.gizzaAI.getAvailableModels()`. Show tool-support badges per model.
- [ ] Conversation persistence — `suppers-ai/messages` already writes to OPFS; the UI just doesn't load prior messages at page init. Wire it up so refresh keeps the thread.
- [ ] Markdown rendering — `marked.js` is already loaded via `ai-bridge.js`. Render assistant content through it in `gizza-app.js` instead of plain text.
- [ ] Code syntax highlighting — add `highlight.js` via CDN after markdown lands.

## Smaller paper-cuts noticed during MVP verification

- [ ] `security-headers` block reads CSP from flow-step config, not from block_configs. Filed in Plan B commit message (`d83d657`). The clean fix is upstream: have the block also consult `block_configs`, or expose `SolobaseBuilder::csp(...)`.
- [ ] Error taxonomy for `AssetLoadError` could split `LoaderNotConfigured` from `UnknownLoader` (noted in Plan A code-quality review) and `Bridge(String)` from `Unknown(String)` (noted in Plan A Task 5 review). Low urgency.
- [ ] `ExternalAsset::timeout_ms` field — today the 120 s timeout in `bridge.js` is hardcoded; ffmpeg-core on slow connections exceeds it. Small addition when ffmpeg lands.
- [ ] Dev server (`python3 -m http.server`) doesn't set `Cache-Control: no-cache` on `sw.js`. Fine for local dev but worth a `_headers` file for GH Pages.

## References

- Design: `docs/superpowers/specs/2026-04-18-gizza-ai-design.md` (workspace sibling)
- Plan A (upstream enablers, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-a-upstream.md`
- Plan B (MVP, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-b-mvp.md`
- Deferred features catalogue: `FUTURE.md`
