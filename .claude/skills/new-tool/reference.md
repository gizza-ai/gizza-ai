# new-tool — reference (per-type files, commands, gotchas)

## Build + test commands (each blocks/<slug>/ and tools/generator are SEPARATE cargo workspaces)
- `cd blocks/<slug> && cargo test --workspace` — core + block unit tests
- `cd blocks/<slug> && wafer build` — wasm32 chat block → target/block.wasm (run from INSIDE the dir; NO path arg). It does NOT generate/update `manifest.json` — that file is scaffold-generated and hand-synced (build.rs requires it).
- `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` — from repo root → web/pkg/<wasm>.js + _bg.wasm
- `cargo run --manifest-path tools/generator/Cargo.toml -- .` — renders pkg/tools/<slug>/
- `solobase build` — rebuild app + all blocks into pkg/
- `cargo install --path cli --force` then `gizza tool <slug> <args>` — CLI test
- `python3 scripts/check-tool-hygiene.py <slug>` — hard gate (CI runs it repo-wide): fails on a
  manifest whose `tool.parameters` drifted from the descriptor (page renders text not `<select>`) or a
  `page/content.md` FAQ written as plain markdown instead of `<details>` accordions.

## Per-type file checklist (fill the scaffold's TODO/stub files)

### pure (reference: blocks/calculator)
- `core/src/lib.rs`: replace `run` with the real fn(s) + at least one happy + one error `#[test]`. Use `f64` (never `i64`) for numeric params.
- `src/lib.rs`: edit `descriptor()` to the real params — it single-sources the chat schema (`parameters = schema_json()` is pre-wired; do NOT hand-write inline JSON) — plus the `skill(description=...)` and `Args` fields; the scaffold delegates via `block_utils::run_skill`. Param API: `Param::string|integer|number|enumv|boolean|string_map(...)` + `.required()/.default(v)/.min(n)/.max(n)/.describe(s)`; `Input::None` for pure. Mirror `blocks/url-encode`.
- `web/src/lib.rs`: `#[wasm_bindgen] pub fn <export>(...) -> Result<T, JsValue>` — the `<export>` name MUST match `page/meta.toml`'s `export`.
- `page/meta.toml`: real slug/title/description/tags/h1/hero_subtitle; `format` = "number"|"text"; one `[[input]] source="field"` per arg — input NAMES + ORDER must equal the web fn's params.
- `page/content.md`: real SEO copy. FAQ as `<details>`/`<summary>` accordions (blank line inside each), never plain `## FAQ` markdown.
- `manifest.json` + `wafer.toml`: scaffold-generated. Update the `summary` in both, and `manifest.json`'s `tool.description`/`tool.parameters` to match your `src/lib.rs` skill() schema. **`tool.parameters` drives the page form** — `control.rs` reads it (not the descriptor) to render `<select>`/checkbox/number/text, so leaving it as the scaffold stub makes every field a text box. Keep it in sync with `schema_json()`.
- `tests/*.json`: wafer fixtures (recipe below).

### ffmpeg (reference: blocks/image-resize)
- `core/src/lib.rs`: `pub fn plan(<params>, in_name: &str) -> Result<(Vec<String>, String), String>` — builds the ffmpeg argv (NO leading "ffmpeg") + `out_name` (keep the input extension). + unit tests.
- `web/src/lib.rs`: `build_argv(<f64 numeric params...>, in_name: &str)` → the shared `gizza_ai_block_utils::ArgvPlan { argv, out_name }` (scaffold default is `build_argv(in_name)`; add the real params). `f64` for numeric (0 = "unset").
- `src/lib.rs`: `descriptor()` = `Input::Image`|`Video` + params (single-sources the `url`⊕`ref` oneOf + schema via `schema_json()`); `run()` = `resolve_source` → `dispatch_ffmpeg` → `build_media_envelope` (the scaffold wires this). Mirror `blocks/image-resize/src/lib.rs`.
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
