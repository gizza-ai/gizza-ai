# new-tool — reference (per-type files, commands, gotchas)

## Build + test commands (each blocks/<slug>/ and tools/generator are SEPARATE cargo workspaces)
- `cd blocks/<slug> && cargo test --workspace` — core + block unit tests
- `cd blocks/<slug> && wafer build` — wasm32 chat block → target/block.wasm (run from INSIDE the dir; NO path arg). It does NOT generate/update `manifest.json` — that file is scaffold-generated and hand-synced (build.rs requires it).
- `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` — from repo root → web/pkg/<wasm>.js + _bg.wasm
- `cargo run --manifest-path tools/generator/Cargo.toml -- .` — renders pkg/tools/<slug>/
- `solobase build` — rebuild app + all blocks into pkg/
- `cargo install --path cli --force` then `gizza tool <slug> <args>` — CLI test
- `python3 scripts/sync-tool-manifest.py <slug>` — AFTER the CLI install: regenerates
  `manifest.json` `tool.parameters`/`tool.description` from the installed CLI's live descriptor
  and propagates the wafer_block macro summary into `manifest.json` + `wafer.toml`. Never
  hand-edit those fields; run this before the generator (which reads the manifest).
- `python3 scripts/check-tool-hygiene.py <slug>` — hard gate (CI runs it repo-wide): fails on a
  manifest whose `tool.parameters` drifted from the descriptor (page renders text not `<select>`), a
  `page/content.md` FAQ written as plain markdown instead of `<details>` accordions, scaffold TODOs,
  or summary drift. Per-slug mode is STRICT and additionally fails on missing field placeholders,
  fewer than 3 FAQ entries, and a meta description outside 50–160 chars.

## Per-type file checklist (fill the scaffold's TODO/stub files)

### pure (reference: blocks/calculator)
- `core/src/lib.rs`: replace `run` with the real fn(s) + at least one happy + one error `#[test]`. Use `f64` (never `i64`) for numeric params.
- `src/lib.rs`: edit `descriptor()` to the real params — it single-sources the chat schema (`parameters = schema_json()` is pre-wired; do NOT hand-write inline JSON) — plus the `skill(description=...)` and `Args` fields; the scaffold delegates via `block_utils::run_skill`. Param API: `Param::string|integer|number|enumv|boolean|string_map(...)` + `.required()/.default(v)/.min(n)/.max(n)/.describe(s)`; `Input::None` for pure. Mirror `blocks/url-encode`.
- `web/src/lib.rs`: `#[wasm_bindgen] pub fn <export>(...) -> Result<T, JsValue>` — the `<export>` name MUST match `page/meta.toml`'s `export`.
- `page/meta.toml`: real slug/title/description/tags/h1/hero_subtitle; `format` = "number"|"text"; one `[[input]] source="field"` per arg — input NAMES + ORDER must equal the web fn's params.
- `page/content.md`: real SEO copy. FAQ as `<details>`/`<summary>` accordions (blank line inside each), never plain `## FAQ` markdown.
- `manifest.json` + `wafer.toml`: scaffold-generated. **`tool.parameters` drives the page form** — `control.rs` reads it (not the descriptor) to render `<select>`/checkbox/number/text, so a scaffold stub makes every field a text box. Do NOT hand-sync: run `python3 scripts/sync-tool-manifest.py <slug>` after the CLI install (it writes `tool.parameters`/`tool.description` from the live descriptor and propagates the macro summary into both files).
- `tests/*.json`: wafer fixtures (recipe below).

### ffmpeg (reference: blocks/image-resize)
- `core/src/lib.rs`: `pub fn plan(<params>, in_name: &str) -> Result<(Vec<String>, String), String>` — builds the ffmpeg argv (NO leading "ffmpeg") + `out_name` (keep the input extension). + unit tests.
- `web/src/lib.rs`: `build_argv(<f64 numeric params...>, in_name: &str)` → the shared `gizza_ai_block_utils::ArgvPlan { argv, out_name }` (scaffold default is `build_argv(in_name)`; add the real params). `f64` for numeric (0 = "unset").
- `src/lib.rs`: `descriptor()` = `Input::Image`|`Video` + params (single-sources the `url`⊕`ref` oneOf + schema via `schema_json()`); `run()` = `resolve_source` → `dispatch_ffmpeg` → `build_media_envelope` (the scaffold wires this). Mirror `blocks/image-resize/src/lib.rs`.
- `page/meta.toml`: `runtime="ffmpeg"`, `[[input]] source="file" accept="image/*"` first, then field inputs in `build_argv` param order, `format="image"`|"video".
- FIELD ORDER in `meta.toml` MUST equal the web `build_argv` param order (`tool.js` passes the field values then `in_name`).

## Playwright spec template (`tests/tool-page-<slug>.spec.ts` — import from './fixtures'; the config serves ../pkg)

Every spec asserts REAL output (an exact value a user would see), never just "something
rendered" — a tool whose transform silently no-ops must FAIL its spec. Always include one
`?param=` deep-link case (the page pre-fills from the query string and auto-runs).

### pure
```ts
import { test, expect } from './fixtures';
test('<slug> page', async ({ page }) => {
  await page.goto('/tools/<slug>/');
  await page.fill('#in-<field>', '<input>');
  await expect(page.locator('#tool-output')).toHaveText('<expected>', { timeout: 15000 });
});
test('<slug> deep-link', async ({ page }) => {
  await page.goto('/tools/<slug>/?<field>=<url-encoded-input>');
  await expect(page.locator('#tool-output')).toHaveText('<expected>', { timeout: 15000 });
});
```

### ffmpeg (needs @ffmpeg CDN network) — §media correctness
A `data:image/` prefix only proves *something* rendered. Decode the output and assert the
transform actually happened: dimensions for resize/crop/rotate/thumbnail, pixel values for
color transforms (grayscale/invert/tint), format for converters (`data:image/webp` etc.).
`fixtures/red-2x2.png` is a known input — pure red, 2×2 — so expected values are exact.
```ts
import { test, expect } from './fixtures';
import * as path from 'path';
test('<slug> page', async ({ page }) => {
  await page.goto('/tools/<slug>/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  // Decode + verify the transform (adapt the assertion to the tool):
  const px = await page.evaluate(async (dataUrl) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = dataUrl; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth; c.height = img.naturalHeight;
    const ctx = c.getContext('2d');
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return { w: img.naturalWidth, h: img.naturalHeight, r: d[0], g: d[1], b: d[2] };
  }, src);
  // resize 2x2 → 1x1:            expect(px.w).toBe(1);
  // grayscale of pure red:       expect(px.r).toBe(px.g); expect(px.g).toBe(px.b);
  // invert of pure red:          expect(px.r).toBeLessThan(30); expect(px.g).toBeGreaterThan(225);
});
```
Video/audio outputs: assert the mime prefix (`data:video/`, `data:audio/`) AND check duration or
size via the element (`media.duration > 0`) after `loadedmetadata` — not just visibility.

## wafer fixture recipe (`tests/*.json`)
`python3 -c "import json;print(list(json.dumps({'input':'...'}).encode()))"` → the byte list goes in `{"kind":"invoke","data":[...],"meta":[]}`.

## Gotchas
- `f64` not `i64` for wasm-bindgen numeric params (else a JS BigInt error at runtime).
- ffmpeg: meta.toml field order = `build_argv` param order.
- Each `blocks/<slug>/` and `tools/generator` are separate workspaces → `cd` into the dir; do NOT use `-p <crate>` from the repo root.
- `wafer build` is run from INSIDE `blocks/<slug>/`.
- **CHAT ffmpeg is non-functional**: the runtime runs in a Service Worker where `import()` and `Worker` are spec-forbidden, so ffmpeg can't run in-chat. ffmpeg tools work via their standalone PAGE + the CLI only. State this in ffmpeg tool PRs.
