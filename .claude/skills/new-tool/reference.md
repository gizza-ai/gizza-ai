# new-tool — reference (per-type files, commands, gotchas)

## Build + test commands (each blocks/<slug>/ and tools/generator are SEPARATE cargo workspaces)
- `cd blocks/<slug> && cargo test --workspace` — core + block unit tests
- `cd blocks/<slug> && wafer build` — wasm32 chat block → target/block.wasm + manifest.json (run from INSIDE the dir; NO path arg)
- `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` — from repo root → web/pkg/<wasm>.js + _bg.wasm
- `cargo run --manifest-path tools/generator/Cargo.toml -- .` — renders pkg/tools/<slug>/
- `solobase build` — rebuild app + all blocks into pkg/
- `cargo install --path cli --force` then `gizza tool <slug> <args>` — CLI test

## Per-type file checklist (fill the scaffold's TODO/stub files)

### pure (reference: blocks/calculator)
- `core/src/lib.rs`: replace `run` with the real fn(s) + at least one happy + one error `#[test]`. Use `f64` (never `i64`) for numeric params.
- `src/lib.rs`: real `skill(description, parameters <JSON-Schema>)` + `Args` fields + delegation to core.
- `web/src/lib.rs`: `#[wasm_bindgen] pub fn <export>(...) -> Result<T, JsValue>` — the `<export>` name MUST match `page/meta.toml`'s `export`.
- `page/meta.toml`: real slug/title/description/tags/h1/hero_subtitle; `format` = "number"|"text"; one `[[input]] source="field"` per arg — input NAMES + ORDER must equal the web fn's params.
- `page/content.md`: real SEO copy.
- `tests/*.json`: wafer fixtures (recipe below).

### ffmpeg (reference: blocks/image-resize)
- `core/src/lib.rs`: `pub fn plan(<params>, in_name: &str) -> Result<(Vec<String>, String), String>` — builds the ffmpeg argv (NO leading "ffmpeg") + `out_name` (keep the input extension). + unit tests.
- `web/src/lib.rs`: `build_argv(<f64 numeric params...>, in_name: &str)` → `{argv, out_name}` (the scaffold's default is `build_argv(in_name)`; add the real params). `f64` for numeric (0 = "unset").
- `src/lib.rs`: skill schema (url/ref + params) — mirror `blocks/image-resize/src/lib.rs` (it dispatches via `dispatch_ffmpeg_runtime`).
- `page/meta.toml`: `runtime="ffmpeg"`, `[[input]] source="file" accept="image/*"` first, then field inputs in `build_argv` param order, `format="image"`|"video".
- FIELD ORDER in `meta.toml` MUST equal the web `build_argv` param order (`tool.js` passes the field values then `in_name`).

## Playwright spec template (`tests/tool-page-<slug>.spec.ts` — import from './fixtures'; the config serves ../pkg)

### pure
```ts
import { test, expect } from './fixtures';
test('<slug> page', async ({ page }) => {
  await page.goto('/tools/<slug>/');
  await page.fill('#in-<field>', '<input>');
  await expect(page.locator('#tool-output')).toHaveText('<expected>', { timeout: 15000 });
});
```

### ffmpeg (needs @ffmpeg CDN network)
```ts
import { test, expect } from './fixtures';
import * as path from 'path';
test('<slug> page', async ({ page }) => {
  await page.goto('/tools/<slug>/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  expect(await media.getAttribute('src')).toMatch(/^data:image\//);
});
```

## wafer fixture recipe (`tests/*.json`)
`python3 -c "import json;print(list(json.dumps({'input':'...'}).encode()))"` → the byte list goes in `{"kind":"invoke","data":[...],"meta":[]}`.

## Gotchas
- `f64` not `i64` for wasm-bindgen numeric params (else a JS BigInt error at runtime).
- ffmpeg: meta.toml field order = `build_argv` param order.
- Each `blocks/<slug>/` and `tools/generator` are separate workspaces → `cd` into the dir; do NOT use `-p <crate>` from the repo root.
- `wafer build` is run from INSIDE `blocks/<slug>/`.
- **CHAT ffmpeg is non-functional**: the runtime runs in a Service Worker where `import()` and `Worker` are spec-forbidden, so ffmpeg can't run in-chat. ffmpeg tools work via their standalone PAGE + the CLI only. State this in ffmpeg tool PRs.
