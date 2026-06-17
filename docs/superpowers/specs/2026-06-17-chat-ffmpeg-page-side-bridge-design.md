# Chat ffmpeg page-side bridge (+ imagine fix, SW-tool audit)

**Date:** 2026-06-17
**Status:** Design approved (Approach D); ready for implementation plan.

## Problem

gizza's chat boots the **entire wafer runtime inside a Service Worker** (`sw.js` →
`initialize()` / `handle_request` run in a `ServiceWorkerGlobalScope`). A
`ServiceWorkerGlobalScope` forbids dynamic `import()` **and** `Worker`
construction. ffmpeg (`@ffmpeg/ffmpeg`) needs both, so the chat's
image/video/ffmpeg tools cannot run there — and never have. This is
pre-existing, not caused by the Phase 0 standalone-tool-page work.

WebLLM already solves the analogous problem: the model runs **page-side** and
postMessages results back to the Service Worker. This design mirrors that escape
for ffmpeg, and audits every other chat tool backed by a `Browser*Service` for
the same wall.

## Audit result (scope correction)

gizza (`src/lib.rs`) wires exactly four browser services:

| Service | Backend | SW-forbidden op | Page engine | Status |
|---|---|---|---|---|
| `BrowserLlmService` | `webllm` | `import()` | `/webllm-engine.js` (loaded) | **Works** |
| `BrowserImageService` (`imagine`) | `transformers-image` | WebGPU | `/t2i-engine.js` (**not loaded**) | **Broken** |
| `BrowserCryptoService` | native | none | none | **Works** (pure Rust) |
| `BrowserFfmpegService` | gizza-local | `import()` + `Worker` | none (no bridge) | **Broken** |

There is **no embedding or vector service registered in gizza** — earlier
"embed/vector" hits were false-positive substrings (`embedded`,
`var_nded_scripts`). So the deliverable is:

- **Build:** the ffmpeg page-side bridge (one-shot).
- **Fix:** `imagine` — one `<script>` tag (its bridge already exists upstream).
- **Audit note (no code):** embeddings is not wired in gizza; crypto is
  native-safe; llm works. If a future tool needs embeddings, load the
  already-served+bypassed `/embed-engine.js` and register a
  `BrowserEmbeddingService` — not in scope now.

## Existing infrastructure (verified)

- **SW message router** — `solobase/crates/solobase-bundle/assets/sw.js.tmpl`
  (lines 81–122) is a **single** `self.addEventListener('message', …)` that
  dispatches page→SW replies by hardcoded prefix (`load-asset-*`, `llm-*`,
  `embed-*-response`, `image-*`) to `globalThis.__solobaseComplete*` resolvers,
  and **`return`s on every unmatched type** (no `stopImmediatePropagation`, no
  throw). A code comment states the reason for the `globalThis` indirection:
  *"bridge.js exposes the resolver on globalThis because this script (sw.js)
  doesn't import the wasm-bindgen-generated bridge module."*
- **Page → SW channel** — the page engines post replies via
  `reg.active?.postMessage(payload)` (`webllm-engine.js` `swPost`), i.e. the
  **global SW message channel**, which fans out to **every**
  `self.addEventListener('message')` registered in the SW — not a dedicated
  `MessageChannel` port.
- **Page engines** — `webllm-engine.js`, `embed-engine.js`, `t2i-engine.js`
  ship from `solobase-bundle`, are present in gizza `pkg/`, and are
  default-listed in the `sw.js.tmpl` fetch-bypass. gizza's `src/blocks/ui.rs`
  (line 305) loads **only** `/webllm-engine.js`.
- **ffmpeg** — `BrowserFfmpegService` (`gizza-ai/src/ffmpeg.rs`) binds
  `#[wasm_bindgen(module = "/js/ffmpeg.js")]` `ffmpegExec(args_json,
  inputs_json, output_name) → { exit_code, output_b64, log }` (one-shot). The
  bound `js/ffmpeg.js` runs in the SW and does `await import(...)` → dies.
  `js/ffmpeg.js` is **gizza-owned** and already works in a page context (the
  standalone tool pages bundle it under `/tools/`).
- **Asset serving** — gizza serves root-path JS via `solobase.toml`
  `[[assets.overlay]]` (`site/X.js → /X.js`) + `extra_bypass_prefix`.

## Decision: Approach D — gizza self-registers its own SW message listener

The page→SW *reply* must reach the SW runtime. gizza's SW-side ffmpeg bridge is a
**wasm-bindgen snippet that already runs inside the SW**, so it simply registers
**its own** `self.addEventListener('message', …)` for `ffmpeg-exec-response` —
no central-router entry, **no solobase-bundle change at all**. This is safe and
not a hack, grounded in the two verified facts above:

1. `sw.js`'s central handler `return`s (and ignores) any unmatched type, with no
   `stopImmediatePropagation` — so a second, independently-registered listener
   fires for the same message without interference.
2. Page replies go over the global SW channel (`reg.active.postMessage`), which
   delivers to **all** SW message listeners — so gizza's listener receives
   `ffmpeg-exec-response` exactly as the central handler receives `llm-*`.

solobase's central-router + `globalThis.__solobaseComplete*` indirection exists
**only because the hand-written `sw.js` can't import the wasm-generated bridge
module**. gizza's snippet has no such limitation (it *is* in the wasm glue, with
`self` = the SW global), so self-registering is the natural fit.

### Rejected alternatives
- **C — generic `__appCompleteBridgeMessage` hook in `sw.js.tmpl`.** Works, but
  needs a solobase-bundle PR + cross-repo pin bump + producer→consumer
  sequencing, and couples apps to a solobase hook — all to replace what a
  one-line self-registered listener does in gizza alone.
- **B — hardcode an `ffmpeg-*` route in `sw.js.tmpl`.** Teaches solobase a
  gizza-only concept and repeats the cross-repo edit per future service.
- **A — migrate llm/embed/image onto one registry.** Touches three working
  shared-infra paths for no benefit here.

D requires **zero solobase-repo change**, collapses to a **single gizza PR**, and
keeps ffmpeg's transport end-to-end owned by the repo that owns ffmpeg.

## Architecture — ffmpeg round-trip

The request originates inside the SW (the tool block runs there), so it must
bounce SW → page → SW:

```
chat LLM → ffmpeg-backed skill block (SW, wasmi)
   → gizza-ai/ffmpeg-runtime FfmpegBlock.handle()        (SW)
   → BrowserFfmpegService::exec()                        (SW, Rust)
   → ffmpeg-bridge.js: post {ffmpeg-exec-request, id} to a window client,
                       await pending[id]                 (SW)
   ── client.postMessage ──▶ ffmpeg-engine.js            (PAGE listener)
                              → import ffmpegExec from /ffmpeg.js  (PAGE: import()/Worker OK)
                              → run ffmpeg
   ◀── reg.active.postMessage {ffmpeg-exec-response, id, ok, result|error}
   → fires ALL SW message listeners:
        · sw.js central handler  → no prefix match → ignored
        · ffmpeg-bridge.js listener → resolve pending[id]      (SW)
   → ExecResult → FfmpegBlock responds → chat renders media
```

`js/ffmpeg.js` becomes the **single source** of ffmpeg execution for both
surfaces: bundled under `/tools/` for the standalone pages, and root-served at
`/ffmpeg.js` for the chat engine to import. No duplicated ffmpeg logic.

## Components / files

### solobase-bundle
**No change.**

### gizza-ai (single PR)
- **`js/ffmpeg-bridge.js` (new, SW-side wasm-bindgen snippet)** — bound via
  `#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]`. At module load it
  registers `self.addEventListener('message', …)` that handles **only**
  `ffmpeg-exec-response` (resolving/rejecting the matching pending entry; every
  other type is ignored with a cheap early return). Also holds the
  pending-request `Map` (`id → {resolve, reject}`), a `postToWindowClient(msg)`
  (`self.clients.matchAll({type:'window'})` → first client; any window client is
  fine — see below), and the request-sender exported as `ffmpegExec(args_json,
  inputs_json, output_name)` (posts `ffmpeg-exec-request`, returns the awaited
  promise). The `ffmpegExec` JS name is unchanged, so the Rust `js_name` binding
  stays identical.
- **`js/ffmpeg-engine.js` (new, page listener)** —
  `navigator.serviceWorker.addEventListener('message', …)` handling
  `ffmpeg-exec-request`: `import { ffmpegExec } from '/ffmpeg.js'`, run it, then
  reply via `reg.active.postMessage({ type:'ffmpeg-exec-response', id, ok,
  result|error })` (mirrors `webllm-engine.js` `swPost`). Wrapped in try/catch so
  it **always** posts a terminal reply.
- **`src/ffmpeg.rs`** — `BrowserFfmpegService` rebinds the extern module from
  `/js/ffmpeg.js` to `/js/ffmpeg-bridge.js`. The `ExecArgs` / `BridgeInput` /
  `BridgeResponse` / `ExecResult` types, the `js_name = ffmpegExec` binding, and
  the base64 in/out handling are **unchanged** — only the JS target moves from
  "run ffmpeg here" to "ask the page." Minimal Rust diff.
- **`src/blocks/ui.rs`** — add (after the `/webllm-engine.js` script):
  `script type="module" src="/ffmpeg-engine.js" {}` and
  `script type="module" src="/t2i-engine.js" {}` (the imagine fix).
- **`solobase.toml`** — `[[assets.overlay]]` entries
  `js/ffmpeg-engine.js → /ffmpeg-engine.js` and `js/ffmpeg.js → /ffmpeg.js`;
  append `"/ffmpeg-engine.js"` and `"/ffmpeg.js"` to `extra_bypass_prefix`.
  (`/t2i-engine.js` is already default-bypassed in `sw.js.tmpl`.) This is
  gizza-owned config in the gizza repo, not a solobase-repo change.

## Message contract (one-shot)

- **request** (SW → page):
  `{ type:'ffmpeg-exec-request', id, args_json, inputs_json, output_name }`
- **response** (page → SW):
  `{ type:'ffmpeg-exec-response', id, exit_code, output_b64, log }`
- `id` — gizza-owned monotonic counter string (`ffmpeg-<n>`; no
  `Date.now()`/random needed).
- The bridge's `ffmpegExec` **always resolves** with `{ exit_code, output_b64,
  log }` and never rejects — so the Rust binding (`ffmpeg_exec(...) -> JsValue`,
  which `.await`s with no rejection handling) stays byte-for-byte unchanged
  except its `module` path. Failures are encoded as `exit_code: -1`,
  `output_b64: ''`, `log: <reason>`.

## Error handling

- ffmpeg nonzero exit / empty output → the real `exit_code` + empty
  `output_b64` flow back through `BridgeResponse` → `FfmpegBlock` (existing
  path; the block already treats this as a failed exec).
- Page exception (ffmpeg threw) → engine encodes `exit_code: -1`, `log: <error>`
  and still posts a terminal response (never a rejected promise).
- No window client available → the SW-side bridge resolves the pending request
  itself with `exit_code: -1, log: 'no window client…'` (don't hang, don't
  reject). No artificial timeout — a long encode must not be killed; the page
  always posts a terminal response.
- `imagine` with no WebGPU → existing graceful `webgpu-unavailable` fallback in
  the imagine block.

## Testing

- **gizza JS unit** (extends existing `js/*.test.js`): bridge pending
  resolve/reject + the self-registered listener (claims `ffmpeg-exec-response`,
  **ignores foreign types** so it never interferes with llm/embed/image);
  engine request→response including the error path; `sw-bypass.test.js` asserts
  `/ffmpeg-engine.js` and `/ffmpeg.js` are bypassed.
- **Playwright e2e** (reuses `tests/` harness): chat invokes the
  `image-grayscale` ffmpeg tool end-to-end → asserts a `data:image/` result
  renders. This is the headline "chat ffmpeg works now" proof.
- **imagine:** Playwright smoke that `/t2i-engine.js` loads and posts
  `image-generate-stream-request`. Full WebGPU generation requires a
  WebGPU-capable browser — headless Chromium may lack it (noted caveat; verify
  the load + request + graceful-fallback path at minimum).

## Deliberate calls

- **Approach D over C/B/A** — zero solobase-repo change, single gizza PR,
  ffmpeg transport co-located with the ffmpeg bridge. Justified by the two
  verified platform facts (unmatched-message fall-through + global-channel
  fan-out).
- **Client selection is not correctness-critical for ffmpeg** — exec is
  stateless and the `id` correlates the reply, so `matchAll({type:'window'})[0]`
  is safe even with multiple tabs. No need to replicate the LLM bridge's client
  selection.
- **One-shot, no progress UI** — a long video encode leaves the chat "thinking"
  for 10–60s with no progress. Acceptable for v1; streamed progress is a future
  option (would need the queue/waiter pattern like the LLM stream).
- **`imagine` is a hypothesis until browser-verified** — its bridge exists but
  was never smoke-tested ("Manual smoke pending"). The plan must actually prove
  the engine loads and posts the request, not assume it.
- **Embeddings is a no-op by evidence** — no embedding service exists in gizza.
