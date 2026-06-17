# Chat ffmpeg page-side bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the gizza chat's ffmpeg-backed tools work by running ffmpeg page-side via a Service-Worker↔page bridge that gizza fully owns; in the same pass, fix the `imagine` tool by loading its already-built page engine.

**Architecture:** The chat boots the whole wafer runtime inside a Service Worker, where `import()`/`Worker` (and thus ffmpeg) are forbidden. `BrowserFfmpegService` (SW, Rust) posts an `ffmpeg-exec-request` to a window client and awaits a correlation-id reply. A page-side `ffmpeg-engine.js` runs the existing `js/ffmpeg.js` (real ffmpeg) and posts an `ffmpeg-exec-response` on the global SW channel (`reg.active.postMessage`), which fans out to every SW `message` listener. The SW-side bridge `js/ffmpeg-bridge.js` (a wasm-bindgen snippet) registers **its own** `self.addEventListener('message', …)` to resolve the pending request — so the framework `sw.js` needs **zero change** (its central handler ignores the unmatched `ffmpeg-*` type). See the design: `docs/superpowers/specs/2026-06-17-chat-ffmpeg-page-side-bridge-design.md`.

**Tech Stack:** Rust (wasm32, `wasm-bindgen`, `async-trait`), vanilla ES modules, `node:test` unit tests, Playwright e2e, `@ffmpeg/ffmpeg` (loaded by the existing `js/ffmpeg.js`).

---

## Why the response is "always resolve, never reject"

The Rust binding `ffmpeg_exec(...) -> JsValue` (in `src/ffmpeg.rs`) `.await`s the JS
promise with **no** rejection handling and JSON-stringifies the result into
`BridgeResponse { exit_code, output_b64, log }`. To keep that Rust code
byte-for-byte unchanged except its module path, the new bridge's `ffmpegExec`
**always resolves** with a `{ exit_code, output_b64, log }` object. Page-side
failures (ffmpeg threw, no window client) are encoded as `exit_code: -1`,
`output_b64: ''`, `log: <reason>` — never a rejected promise.

## Message contract

- **request** (SW → page): `{ type:'ffmpeg-exec-request', id, args_json, inputs_json, output_name }`
- **response** (page → SW): `{ type:'ffmpeg-exec-response', id, exit_code, output_b64, log }`
- `id` — a gizza-owned monotonic counter string (`ffmpeg-<n>`).

## File structure

| File | Responsibility |
|---|---|
| `js/ffmpeg-engine.js` (new) | **Page-side.** Listen for `ffmpeg-exec-request`, run real ffmpeg via `/ffmpeg.js`, post `ffmpeg-exec-response`. |
| `js/ffmpeg-bridge.js` (new) | **SW-side wasm-bindgen snippet.** Pending-id registry + `ffmpegExec` request-sender + self-registered `message` listener that resolves the pending request. |
| `src/ffmpeg.rs` (modify) | Rebind `BrowserFfmpegService`'s extern module from `/js/ffmpeg.js` → `/js/ffmpeg-bridge.js`. Nothing else changes. |
| `src/blocks/ui.rs` (modify) | Load `/ffmpeg-engine.js` (chat ffmpeg) and `/t2i-engine.js` (imagine) script tags. |
| `solobase.toml` (modify) | Overlay `js/ffmpeg-engine.js`→`/ffmpeg-engine.js` and `js/ffmpeg.js`→`/ffmpeg.js`; add both to `extra_bypass_prefix`. |
| `js/ffmpeg-engine.test.js` (new) | Unit: `runFfmpegRequest` success + never-throws error path. |
| `js/ffmpeg-bridge.test.js` (new) | Unit: id correlation, foreign-type/unknown-id ignore, `ffmpegExec` resolve. |
| `js/ffmpeg-roundtrip.test.js` (new) | Integration: bridge↔engine request→response correlation (fake ffmpeg). |
| `js/sw-bypass.test.js` (modify) | Assert `/ffmpeg-engine.js` + `/ffmpeg.js` are SW-bypassed. |
| `tests/chat-ffmpeg-bridge.spec.ts` (new) | Browser e2e: real ffmpeg runs in the chat page through `ffmpeg-engine.js`. |

## Commands (run from `gizza-ai/` unless noted)

- One JS test file: `node --test js/ffmpeg-engine.test.js`
- All JS tests (CI gate): `npm test`
- Rust tests (CI gate): `cargo test`
- wasm compile check: `cargo build --target wasm32-unknown-unknown`
- Full app build (assembles `pkg/`, regenerates `pkg/sw.js`, copies overlays): `solobase build`
  - Cold prereq (once): in `../solobase`, `wasm-pack build crates/solobase-web --target web --release --out-dir pkg`; then `cargo install --path ../solobase/crates/solobase --locked` and `cargo install --path ../wafer-run/crates/wafer-cli --locked`.
- Browser e2e (not in CI — needs network + a browser): `cd tests && npm install && npx playwright test chat-ffmpeg-bridge.spec.ts`

---

### Task 1: Page-side ffmpeg engine (`js/ffmpeg-engine.js`)

**Files:**
- Create: `js/ffmpeg-engine.js`
- Test: `js/ffmpeg-engine.test.js`

- [ ] **Step 1: Write the failing test**

Create `js/ffmpeg-engine.test.js`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runFfmpegRequest } from './ffmpeg-engine.js';

test('runFfmpegRequest returns the exec result on success', async () => {
  const fakeExec = async (a, i, o) => {
    assert.equal(a, '["-i","in.png","out.png"]');
    assert.equal(i, '[{"name":"in.png","bytes_b64":"AAA"}]');
    assert.equal(o, 'out.png');
    return { exit_code: 0, output_b64: 'QkJC', log: 'ok' };
  };
  const resp = await runFfmpegRequest(
    {
      args_json: '["-i","in.png","out.png"]',
      inputs_json: '[{"name":"in.png","bytes_b64":"AAA"}]',
      output_name: 'out.png',
    },
    fakeExec,
  );
  assert.deepEqual(resp, { exit_code: 0, output_b64: 'QkJC', log: 'ok' });
});

test('runFfmpegRequest encodes a thrown error as exit_code -1 (never throws)', async () => {
  const boom = async () => { throw new Error('worker died'); };
  const resp = await runFfmpegRequest(
    { args_json: '[]', inputs_json: '[]', output_name: 'out.png' },
    boom,
  );
  assert.equal(resp.exit_code, -1);
  assert.equal(resp.output_b64, '');
  assert.match(resp.log, /worker died/);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test js/ffmpeg-engine.test.js`
Expected: FAIL — `Cannot find module '.../js/ffmpeg-engine.js'`.

- [ ] **Step 3: Write the implementation**

Create `js/ffmpeg-engine.js`:

```js
// ffmpeg-engine.js — gizza-ai chat ffmpeg page-side engine.
//
// Runs in the window, where dynamic import() and Worker — both forbidden in a
// ServiceWorkerGlobalScope — are allowed. Listens for `ffmpeg-exec-request`
// messages the SW posts (see js/ffmpeg-bridge.js), runs the real ffmpeg via the
// shared js/ffmpeg.js, and posts an `ffmpeg-exec-response` back on the global SW
// channel (reg.active.postMessage) — the same channel webllm-engine.js uses.

// Lazy + injectable so node unit tests can drive runFfmpegRequest without a real
// /ffmpeg.js (which dynamically imports the @ffmpeg CDN bundle).
async function defaultExec(argsJson, inputsJson, outputName) {
  const mod = await import('/ffmpeg.js');
  return mod.ffmpegExec(argsJson, inputsJson, outputName);
}

// Run one ffmpeg request and return a BridgeResponse-shaped result. Never
// throws: page-side failures are encoded as exit_code -1 + log so the Rust
// BrowserFfmpegService (which never handles a rejected promise) stays unchanged.
export async function runFfmpegRequest(msg, exec = defaultExec) {
  try {
    const r = await exec(msg.args_json, msg.inputs_json, msg.output_name);
    return { exit_code: r.exit_code, output_b64: r.output_b64, log: r.log };
  } catch (e) {
    return { exit_code: -1, output_b64: '', log: String(e) };
  }
}

async function swPost(payload) {
  const reg = await navigator.serviceWorker.ready;
  reg.active?.postMessage(payload);
}

// Register the page-side listener only in a real browser (skipped under node,
// where navigator is undefined).
if (typeof navigator !== 'undefined' && navigator.serviceWorker) {
  navigator.serviceWorker.addEventListener('message', async (event) => {
    const msg = event.data;
    if (!msg || msg.type !== 'ffmpeg-exec-request') return;
    const resp = await runFfmpegRequest(msg);
    await swPost({ type: 'ffmpeg-exec-response', id: msg.id, ...resp });
  });
  console.log('ffmpeg-engine.js loaded');
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test js/ffmpeg-engine.test.js`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add js/ffmpeg-engine.js js/ffmpeg-engine.test.js
git commit -m "feat(chat-ffmpeg): page-side ffmpeg-engine.js (runs real ffmpeg, replies to SW)"
```

---

### Task 2: SW-side ffmpeg bridge (`js/ffmpeg-bridge.js`)

**Files:**
- Create: `js/ffmpeg-bridge.js`
- Test: `js/ffmpeg-bridge.test.js`

- [ ] **Step 1: Write the failing test**

Create `js/ffmpeg-bridge.test.js`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  makeId,
  registerPending,
  completeBridgeMessage,
  ffmpegExec,
} from './ffmpeg-bridge.js';

test('makeId returns unique ffmpeg-prefixed ids', () => {
  const a = makeId();
  const b = makeId();
  assert.match(a, /^ffmpeg-\d+$/);
  assert.notEqual(a, b);
});

test('completeBridgeMessage resolves the matching pending request', async () => {
  const p = registerPending('ffmpeg-test-1');
  const consumed = completeBridgeMessage({
    type: 'ffmpeg-exec-response',
    id: 'ffmpeg-test-1',
    exit_code: 0,
    output_b64: 'ZZZ',
    log: 'done',
  });
  assert.equal(consumed, true);
  assert.deepEqual(await p, { exit_code: 0, output_b64: 'ZZZ', log: 'done' });
});

test('completeBridgeMessage ignores foreign types and unknown ids', () => {
  assert.equal(completeBridgeMessage({ type: 'llm-stream-frame', id: 'x' }), false);
  assert.equal(completeBridgeMessage({ type: 'ffmpeg-exec-response', id: 'no-such-id' }), false);
});

test('ffmpegExec posts a request and resolves when the response arrives', async () => {
  let posted = null;
  const fakePost = async (msg) => { posted = msg; };
  const resultPromise = ffmpegExec('["-i","in","out"]', '[]', 'out', fakePost);
  assert.equal(posted.type, 'ffmpeg-exec-request');
  assert.equal(posted.args_json, '["-i","in","out"]');
  assert.equal(posted.output_name, 'out');
  completeBridgeMessage({
    type: 'ffmpeg-exec-response',
    id: posted.id,
    exit_code: 0,
    output_b64: 'QQ',
    log: '',
  });
  assert.deepEqual(await resultPromise, { exit_code: 0, output_b64: 'QQ', log: '' });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test js/ffmpeg-bridge.test.js`
Expected: FAIL — `Cannot find module '.../js/ffmpeg-bridge.js'`.

- [ ] **Step 3: Write the implementation**

Create `js/ffmpeg-bridge.js`:

```js
// ffmpeg-bridge.js — gizza-ai chat ffmpeg SW-side bridge.
//
// Runs inside the Service Worker (imported as a wasm-bindgen snippet by
// BrowserFfmpegService). ffmpeg can't run in a ServiceWorkerGlobalScope
// (import()/Worker forbidden), so this posts an `ffmpeg-exec-request` to a
// window client and awaits the matching `ffmpeg-exec-response`. The page
// (js/ffmpeg-engine.js) replies on the global SW channel, which fans out to
// every SW `message` listener — including the one this module registers below.
// The framework sw.js central handler ignores the unmatched `ffmpeg-*` type.
//
// `ffmpegExec` keeps the exact name + (argsJson, inputsJson, outputName)
// signature the Rust binding expects, and ALWAYS RESOLVES with a
// { exit_code, output_b64, log } object (errors encoded as exit_code -1) so the
// Rust side stays unchanged except its module path.

const pending = new Map(); // id -> { resolve }
let _counter = 0;

export function makeId() {
  return `ffmpeg-${++_counter}`;
}

export function registerPending(id) {
  return new Promise((resolve) => { pending.set(id, { resolve }); });
}

// Resolve the pending request matching an `ffmpeg-exec-response`. Returns true
// iff this module consumed the message; foreign types and unknown ids are
// ignored so the central sw.js routing is never disturbed.
export function completeBridgeMessage(msg) {
  if (!msg || msg.type !== 'ffmpeg-exec-response') return false;
  const entry = pending.get(msg.id);
  if (!entry) return false;
  pending.delete(msg.id);
  entry.resolve({ exit_code: msg.exit_code, output_b64: msg.output_b64, log: msg.log });
  return true;
}

async function postToWindowClient(msg) {
  const clients = await self.clients.matchAll({ type: 'window' });
  if (!clients.length) {
    // No page to run ffmpeg — fail the pending request through the same path
    // (any window client is fine; the id correlates the reply).
    completeBridgeMessage({
      type: 'ffmpeg-exec-response',
      id: msg.id,
      exit_code: -1,
      output_b64: '',
      log: 'no window client available to run ffmpeg',
    });
    return;
  }
  clients[0].postMessage(msg);
}

// Called by Rust BrowserFfmpegService. `post` is injectable for tests; in the
// SW it defaults to posting to a window client.
export async function ffmpegExec(argsJson, inputsJson, outputName, post = postToWindowClient) {
  const id = makeId();
  const result = registerPending(id);
  await post({
    type: 'ffmpeg-exec-request',
    id,
    args_json: argsJson,
    inputs_json: inputsJson,
    output_name: outputName,
  });
  return result;
}

// Register our own SW message listener (skipped under node, where self is
// undefined). Multiple message listeners coexist; sw.js's central handler
// ignores `ffmpeg-exec-response`, so this is the only consumer.
if (typeof self !== 'undefined' && typeof self.addEventListener === 'function') {
  self.addEventListener('message', (event) => { completeBridgeMessage(event?.data); });
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test js/ffmpeg-bridge.test.js`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add js/ffmpeg-bridge.js js/ffmpeg-bridge.test.js
git commit -m "feat(chat-ffmpeg): SW-side ffmpeg-bridge.js (self-registered message listener)"
```

---

### Task 3: Bridge↔engine round-trip integration test

**Files:**
- Test: `js/ffmpeg-roundtrip.test.js`

- [ ] **Step 1: Write the test (passes immediately — both modules exist)**

Create `js/ffmpeg-roundtrip.test.js`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ffmpegExec, completeBridgeMessage } from './ffmpeg-bridge.js';
import { runFfmpegRequest } from './ffmpeg-engine.js';

// Wire the SW-side bridge to the page-side engine with fakes for the two
// postMessage hops and a fake ffmpeg, proving request→response correlation end
// to end without a browser or real ffmpeg.
test('bridge request round-trips through the engine and resolves', async () => {
  const fakeFfmpeg = async (argsJson, inputsJson, outputName) => {
    assert.equal(outputName, 'out.png');
    return { exit_code: 0, output_b64: 'R1JBWQ', log: 'frames' };
  };

  // SW→page hop: when the bridge "posts" a request, run the engine on it…
  const post = async (request) => {
    const resp = await runFfmpegRequest(request, fakeFfmpeg);
    // …page→SW hop: feed the engine's response back into the bridge.
    completeBridgeMessage({ type: 'ffmpeg-exec-response', id: request.id, ...resp });
  };

  const result = await ffmpegExec(
    '["-i","in.png","-vf","format=gray","out.png"]',
    '[{"name":"in.png","bytes_b64":"AAAA"}]',
    'out.png',
    post,
  );
  assert.deepEqual(result, { exit_code: 0, output_b64: 'R1JBWQ', log: 'frames' });
});
```

- [ ] **Step 2: Run test to verify it passes**

Run: `node --test js/ffmpeg-roundtrip.test.js`
Expected: PASS — 1 test.

- [ ] **Step 3: Run the full JS suite (no regressions)**

Run: `npm test`
Expected: PASS — all `js/*.test.js` including the new three.

- [ ] **Step 4: Commit**

```bash
git add js/ffmpeg-roundtrip.test.js
git commit -m "test(chat-ffmpeg): bridge<->engine round-trip integration test"
```

---

### Task 4: Rebind `BrowserFfmpegService` to the bridge

**Files:**
- Modify: `src/ffmpeg.rs:9-10` (doc), `src/ffmpeg.rs:24` (module path), `src/ffmpeg.rs:30-32` (doc)

- [ ] **Step 1: Change the wasm-bindgen module path**

In `src/ffmpeg.rs`, change line 24 from:

```rust
#[wasm_bindgen(module = "/js/ffmpeg.js")]
```

to:

```rust
#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]
```

- [ ] **Step 2: Update the two doc comments that name the old module**

In `src/ffmpeg.rs`, change lines 7-10 from:

```rust
//! This file keeps only the browser-side implementation: `BrowserFfmpegService`
//! and its `BridgeInput`/`BridgeResponse` helpers. `BrowserFfmpegService` uses
//! `#[wasm_bindgen(module = "/js/ffmpeg.js")]`, which is resolved relative to
//! THIS crate's root and must stay here.
```

to:

```rust
//! This file keeps only the browser-side implementation: `BrowserFfmpegService`
//! and its `BridgeInput`/`BridgeResponse` helpers. `BrowserFfmpegService` uses
//! `#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]`, which posts the work to a
//! window client (ffmpeg can't run in the Service Worker) and is resolved
//! relative to THIS crate's root, so it must stay here.
```

And change lines 30-32 from:

```rust
/// Browser-side ffmpeg service. Uses `@ffmpeg/ffmpeg` from jsdelivr via
/// the wasm-bindgen module at `js/ffmpeg.js`. wasm32-only — native tests
/// substitute their own `FfmpegService` impl.
```

to:

```rust
/// Browser-side ffmpeg service. Delegates to a window client via the
/// wasm-bindgen bridge at `js/ffmpeg-bridge.js`, which postMessages a page
/// running `@ffmpeg/ffmpeg` (the Service Worker can't run ffmpeg itself).
/// wasm32-only — native tests substitute their own `FfmpegService` impl.
```

- [ ] **Step 3: Verify Rust still compiles (native + wasm)**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: builds clean (the extern resolves to the new snippet at wasm-bindgen time during `solobase build`; `cargo build` validates the Rust).

Run: `cargo test`
Expected: PASS — `src/ffmpeg.rs` is `#[cfg(target_arch = "wasm32")]`-gated, so native tests are unaffected; nothing regresses.

- [ ] **Step 4: Commit**

```bash
git add src/ffmpeg.rs
git commit -m "feat(chat-ffmpeg): BrowserFfmpegService delegates to js/ffmpeg-bridge.js"
```

---

### Task 5: Load the page engines in the chat UI (chat ffmpeg + imagine)

**Files:**
- Modify: `src/blocks/ui.rs:305` (add two script tags), `src/blocks/ui.rs` test module (~343)

- [ ] **Step 1: Write the failing test**

In `src/blocks/ui.rs`, inside `#[cfg(test)] mod tests` (after the existing `renders_composer_disclaimer` test, before the closing `}`), add:

```rust
    #[test]
    fn loads_page_side_engines_for_chat_ffmpeg_and_imagine() {
        let s = render_chat().into_string();
        assert!(
            s.contains(r#"src="/ffmpeg-engine.js""#),
            "chat ffmpeg page engine loaded"
        );
        assert!(
            s.contains(r#"src="/t2i-engine.js""#),
            "imagine (t2i) page engine loaded"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test loads_page_side_engines_for_chat_ffmpeg_and_imagine`
Expected: FAIL — neither script src is present yet.

- [ ] **Step 3: Add the two script tags**

In `src/blocks/ui.rs`, change line 305 from:

```rust
                script type="module" src="/webllm-engine.js" {}
                script type="module" src="/gizza-app.js" {}
```

to:

```rust
                script type="module" src="/webllm-engine.js" {}
                // Page-side text-to-image (imagine) engine — image-* bridge
                // already exists upstream; loading it is the whole fix.
                script type="module" src="/t2i-engine.js" {}
                // Page-side chat ffmpeg engine — runs ffmpeg in the window and
                // replies to the SW (see js/ffmpeg-bridge.js / js/ffmpeg-engine.js).
                script type="module" src="/ffmpeg-engine.js" {}
                script type="module" src="/gizza-app.js" {}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test loads_page_side_engines_for_chat_ffmpeg_and_imagine`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/blocks/ui.rs
git commit -m "feat(chat-ffmpeg): load /ffmpeg-engine.js + /t2i-engine.js (imagine) in chat UI"
```

---

### Task 6: Serve + SW-bypass the new assets

**Files:**
- Modify: `js/sw-bypass.test.js` (add assertions), `solobase.toml` (`[[assets.overlay]]` ×2 + `extra_bypass_prefix`)

- [ ] **Step 1: Write the failing bypass assertions**

In `js/sw-bypass.test.js`, after the existing `/tools/` test, append:

```js
test('sw.js bypasses /ffmpeg-engine.js and /ffmpeg.js so the chat ffmpeg bridge loads statically', () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/ffmpeg-engine\.js['"]\)/,
    'sw.js is missing the /ffmpeg-engine.js bypass — check extra_bypass_prefix in solobase.toml',
  );
  assert.match(
    src,
    /startsWith\(['"]\/ffmpeg\.js['"]\)/,
    'sw.js is missing the /ffmpeg.js bypass — check extra_bypass_prefix in solobase.toml',
  );
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test js/sw-bypass.test.js`
Expected: FAIL — current `pkg/sw.js` has neither bypass entry.

- [ ] **Step 3: Add the overlays**

In `solobase.toml`, add two `[[assets.overlay]]` blocks (next to the other overlays):

```toml
[[assets.overlay]]
from = "js/ffmpeg-engine.js"
to = "ffmpeg-engine.js"

[[assets.overlay]]
from = "js/ffmpeg.js"
to = "ffmpeg.js"
```

- [ ] **Step 4: Add the bypass prefixes**

In `solobase.toml`, append `"/ffmpeg-engine.js"` and `"/ffmpeg.js"` to the `extra_bypass_prefix` array under `[assets]` (keep the existing entries, add at the end before `]`):

```toml
extra_bypass_prefix = ["/gizza-app.js", "/gizza.css", "/render.js", "/pending.js", "/gis.png", "/gis_no_eyes.png", "/gis_a_job_no_eyes.png", "/eye.png", "/gis_video_idle.mp4", "/gis_video_typing_loop.mp4", "/gis_video_typing_finish.mp4", "/favicon.ico", "/favicon-32.png", "/apple-touch-icon.png", "/logo.webp", "/model-picker.js", "/model-picker.css", "/tool.js", "/tool.css", "/tools/", "/tools-modal.js", "/tools-modal.css", "/ffmpeg-engine.js", "/ffmpeg.js"]
```

- [ ] **Step 5: Rebuild so `pkg/sw.js` + overlays regenerate**

Run: `solobase build`
Expected: build succeeds; this is also the first **full** validation that the
`#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]` snippet wiring from Task 4
resolves (wasm-bindgen copies `js/ffmpeg-bridge.js` into `pkg/snippets/`), and it
copies `ffmpeg-engine.js` + `ffmpeg.js` into `pkg/`.

- [ ] **Step 6: Run the bypass test to verify it passes**

Run: `node --test js/sw-bypass.test.js`
Expected: PASS — both new assertions match the regenerated `pkg/sw.js`.

- [ ] **Step 7: Verify the assets landed in `pkg/`**

Run: `ls pkg/ffmpeg-engine.js pkg/ffmpeg.js`
Expected: both files exist.

- [ ] **Step 8: Commit**

```bash
git add solobase.toml js/sw-bypass.test.js
git commit -m "build(chat-ffmpeg): serve + SW-bypass /ffmpeg-engine.js and /ffmpeg.js"
```

---

### Task 7: Browser e2e — real ffmpeg in the chat page

**Files:**
- Create: `tests/chat-ffmpeg-bridge.spec.ts` (reuses `tests/fixtures/red-2x2.png`)

- [ ] **Step 1: Write the e2e test**

Create `tests/chat-ffmpeg-bridge.spec.ts`:

```ts
import { test, expect } from './fixtures';
import * as fs from 'fs';
import * as path from 'path';

// Proves real ffmpeg runs in the CHAT page context (where import()/Worker work,
// unlike the Service Worker) via ffmpeg-engine.js. The SW<->page postMessage
// plumbing is covered by the node round-trip unit test; the full LLM-driven
// chat path is a manual smoke (see the plan's Task 8 notes).
test('chat page runs ffmpeg page-side via ffmpeg-engine.js', async ({ page }) => {
  test.setTimeout(120_000); // first run downloads the @ffmpeg core from the CDN

  await page.goto('/');
  await expect(page.locator('h1')).toContainText(/gizza/i, { timeout: 30_000 });

  const pngB64 = fs
    .readFileSync(path.resolve(__dirname, 'fixtures/red-2x2.png'))
    .toString('base64');

  const resp = await page.evaluate(async (b64) => {
    const m = await import('/ffmpeg-engine.js');
    const inputs_json = JSON.stringify([{ name: 'in.png', bytes_b64: b64 }]);
    const args_json = JSON.stringify(['-i', 'in.png', '-vf', 'format=gray', 'out.png']);
    return await m.runFfmpegRequest({ args_json, inputs_json, output_name: 'out.png' });
  }, pngB64);

  expect(resp.exit_code).toBe(0);
  expect(resp.output_b64.length).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Ensure `pkg/` is built (from Task 6) and run the e2e**

Run: `cd tests && npm install && npx playwright test chat-ffmpeg-bridge.spec.ts`
Expected: PASS — `exit_code` is 0 and `output_b64` is non-empty (real grayscale PNG produced in the chat page).

- [ ] **Step 3: Commit**

```bash
git add tests/chat-ffmpeg-bridge.spec.ts
git commit -m "test(chat-ffmpeg): e2e proving real ffmpeg runs in the chat page via the engine"
```

---

### Task 8: Final verification, imagine smoke, audit note

**Files:**
- Modify: `docs/superpowers/specs/2026-06-17-chat-ffmpeg-page-side-bridge-design.md` (mark verified)

- [ ] **Step 1: Run the full CI-gate suites**

Run (from `gizza-ai/`): `cargo test && npm test`
Expected: all Rust + all JS tests PASS.

- [ ] **Step 2: Manual smoke — chat ffmpeg end-to-end (not CI; needs WebGPU + network)**

In a real browser against a `solobase build` of the app: load a model in the chat,
then invoke an ffmpeg-backed tool (e.g. `/image-grayscale` with an uploaded
image, or ask the model to grayscale an image). Confirm a media result renders
and the SW does **not** log `import() is disallowed on ServiceWorkerGlobalScope`.
Record the result (pass/fail) in the spec's status. If WebLLM/WebGPU is
unavailable in the test browser, note that this smoke could not be run.

- [ ] **Step 3: Manual smoke — imagine (T2I), WebGPU-dependent**

In the same browser, invoke `/imagine <prompt>`. Confirm `/t2i-engine.js` loads
(DevTools Network → 200) and an `image-generate-stream-request` is posted. On a
WebGPU-capable browser, confirm an image is produced; on one without WebGPU,
confirm the graceful `webgpu-unavailable` message (not a crash). Record the
outcome in the spec.

- [ ] **Step 4: Record the audit outcome in the spec**

In `docs/superpowers/specs/2026-06-17-chat-ffmpeg-page-side-bridge-design.md`,
update the **Status** line to note: ffmpeg bridge implemented + tested; imagine
engine loaded; embeddings confirmed a no-op (no service wired); crypto/llm
unaffected. Add the manual-smoke results from Steps 2-3.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-06-17-chat-ffmpeg-page-side-bridge-design.md
git commit -m "docs(chat-ffmpeg): record implementation + smoke results in the spec"
```

---

## Self-review

**Spec coverage:**
- ffmpeg page-side bridge (one-shot) → Tasks 1-4, 6, 7. ✓
- Approach D, zero solobase change, gizza self-registered listener → Task 2 (`self.addEventListener`), Task 4 (rebind only). No `sw.js.tmpl` edit anywhere. ✓
- imagine fix (load `/t2i-engine.js`) → Task 5 + Task 8 Step 3. ✓
- Serve + bypass `/ffmpeg-engine.js` + `/ffmpeg.js` → Task 6. ✓
- Embeddings no-op / crypto / llm audit → Task 8 Step 4 (doc). ✓
- Single source `js/ffmpeg.js` (tool pages + chat) → Task 1 imports `/ffmpeg.js`; Task 6 overlays it. ✓
- Testing: JS unit + round-trip + sw-bypass (CI) + browser e2e + manual smokes → Tasks 1-3, 6, 7, 8. ✓

**Placeholder scan:** No TBD/TODO/"add error handling"/"similar to" — every code and command step is concrete. ✓

**Type/name consistency:** `runFfmpegRequest(msg, exec)`, `ffmpegExec(argsJson, inputsJson, outputName, post)`, `makeId`, `registerPending`, `completeBridgeMessage` are used identically across Tasks 1-3 and the e2e. The message types (`ffmpeg-exec-request` / `ffmpeg-exec-response`) and the `{ exit_code, output_b64, log }` shape match across bridge, engine, tests, and the Rust `BridgeResponse`. The Rust `js_name = ffmpegExec` binding (unchanged) matches the bridge's exported `ffmpegExec`. ✓

**Known coverage gap (stated, not hidden):** the full Rust→SW→page→SW chain is only exercised by the LLM-driven manual smoke (Task 8 Step 2); the deterministic CI tests cover each half (node round-trip for the plumbing, browser e2e for real ffmpeg). This is by design — the only HTTP entry to tools is the non-deterministic `POST /b/agent/chat`.
