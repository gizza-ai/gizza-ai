# Media-I/O Page Extension (Phase 0) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the gizza standalone-tool-page system to support **file input + media output** so ffmpeg tools get real, Playwright-testable pages, and prove it by giving `image-resize` a page.

**Architecture:** Today `tools/generator/` + `site/tool.js` + `page/meta.toml` only handle pure text→number/text compute. We add a `runtime="ffmpeg"` page mode: a `file` input source, an `image`/`video` output format, and a `tool.js` branch that reads the uploaded file, asks the tool's `web/` wasm to build the ffmpeg **argv** (pure, shared from `core/`), runs it via the existing header-free single-threaded browser ffmpeg (`js/ffmpeg.js` → `ffmpegExec`), and renders the result. `image-resize` is refactored to the `core/`+`web/`+`page/` shape (its `build_argv` becomes the single source of truth) as the reference + proof.

**Tech Stack:** Rust (the `tools/generator` maud renderer + `image-resize` block), `wasm-bindgen` (`web/`), vanilla JS (`site/tool.js`), `@ffmpeg/ffmpeg`@0.12 single-threaded (already wired in `js/ffmpeg.js`), Playwright (page test).

**Spec:** `docs/superpowers/specs/2026-06-17-gizza-new-tool-skill-design.md` (Phase 0).

---

## Key facts (verified, paste-ready)

- `js/ffmpeg.js` exports `async function ffmpegExec(argsJson, inputsJson, outputName) -> {exit_code, output_b64, log}`. `argsJson` = JSON array of strings (ffmpeg argv WITHOUT the leading "ffmpeg"); `inputsJson` = JSON array of `{name, bytes_b64}`; `outputName` = the virtual-FS file to read back. It loads the **single-threaded** `@ffmpeg/core` UMD build — no COOP/COEP needed, runs in headless Chromium.
- `site/tool.js` reads `window.GIZZA_TOOL` (`{module, export, live, intervalMs, inputs:[{name,source,elementId}], output:{label,elementId}, format}`), imports the wasm module, wires field inputs to `cfg.export`, renders text/number into `#tool-output`.
- `tools/generator/src/meta.rs` `ToolMeta` is parsed from `page/meta.toml`; `client_config()` builds `window.GIZZA_TOOL`. `template.rs::render_page` emits the HTML; `main.rs` copies `web/pkg/<wasm>.js`, `<wasm>_bg.wasm`, `site/tool.js`, `site/tool.css` into `pkg/tools/<slug>/`.
- `image-resize` is currently `[workspace] members=["."]` (single crate). Its `src/lib.rs` has `enum Fit{Contain,Cover,Stretch}`, `parse_fit(Option<&str>)->Result<Fit,String>`, `build_argv(in_name,out_name,w:Option<u32>,h:Option<u32>,fit:Fit)->Vec<String>`, and `run()` (fetch source → build argv → `dispatch_ffmpeg_runtime` → envelope). `build_argv`/`Fit`/`parse_fit` are pure and already have unit tests.
- `calculator` is the 3-crate template: `[workspace] members=[".","core","web"]`; `core/` is pure (no wafer/wasm-bindgen); `web/` is a `cdylib` with `wasm-bindgen` calling `core`.

---

## File structure

```
gizza-ai/
  tools/generator/src/meta.rs       # MODIFY: + runtime, Input.accept; client_config emits them
  tools/generator/src/template.rs   # MODIFY: render file inputs + media output
  tools/generator/src/main.rs       # MODIFY: copy js/ffmpeg.js for runtime="ffmpeg"
  site/tool.js                      # MODIFY: add the ffmpeg branch (file → build_argv → ffmpegExec → media)
  site/tool.css                     # MODIFY: + .tool-file, .tool-output-media styles
  js/tool-ffmpeg.test.js            # CREATE: unit tests for the pure ffmpeg helpers
  blocks/image-resize/Cargo.toml    # MODIFY: members = [".","core","web"]
  blocks/image-resize/core/Cargo.toml      # CREATE
  blocks/image-resize/core/src/lib.rs      # CREATE: Fit/parse_fit/build_argv/out_name (+ tests, moved from src)
  blocks/image-resize/web/Cargo.toml       # CREATE
  blocks/image-resize/web/src/lib.rs       # CREATE: #[wasm_bindgen] build_argv
  blocks/image-resize/src/lib.rs    # MODIFY: use core::{Fit,parse_fit,build_argv}; delete the moved items
  blocks/image-resize/page/meta.toml       # CREATE: runtime=ffmpeg, file input, image output
  blocks/image-resize/page/content.md      # CREATE
  tests/fixtures/red-2x2.png        # CREATE: tiny test image for Playwright
  tests/tool-page-image-resize.spec.ts     # CREATE: Playwright page test (or .mjs per repo convention)
```

---

## Milestone A — page-system extension (no image-resize yet)

### Task 1: `meta.rs` — `runtime` field + `Input.accept` + client_config

**Files:** Modify `tools/generator/src/meta.rs`

- [ ] **Step 1: Add the failing test** (append to `meta.rs` `mod tests`)

```rust
    #[test]
    fn parses_ffmpeg_meta_with_file_input() {
        let text = r#"
slug          = "image-resize"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "gizza_ai_image_resize_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Resized image"
format        = "image"

[[input]]
name   = "image"
source = "file"
accept = "image/*"
label  = "Image"

[[input]]
name   = "width"
source = "field"
label  = "Width (px)"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.runtime, "ffmpeg");
        assert_eq!(m.inputs[0].source, "file");
        assert_eq!(m.inputs[0].accept, "image/*");
        let cfg = m.client_config();
        assert_eq!(cfg["runtime"], "ffmpeg");
        assert_eq!(cfg["inputs"][0]["accept"], "image/*");
        assert_eq!(cfg["format"], "image");
    }

    #[test]
    fn runtime_defaults_to_wasm() {
        // existing calculator meta has no `runtime` key
        let text = r#"
slug = "calculator"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "evaluate"
output_label = "Result"
format = "number"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.runtime, "wasm");
    }
```

- [ ] **Step 2: Run to verify failure** — `cd gizza-ai && cargo test -p gizza-tool-pages parses_ffmpeg_meta_with_file_input runtime_defaults_to_wasm`
Expected: FAIL (no `runtime` field / no `accept`). (If `-p gizza-tool-pages` errors, the package name is in `tools/generator/Cargo.toml` — use it; or `cd tools/generator && cargo test`.)

- [ ] **Step 3: Implement** — in `meta.rs`:

Add `accept` to `Input`:
```rust
pub struct Input {
    pub name: String,
    /// "field" = visible text input; "clock" = unix seconds from JS; "file" = file picker.
    pub source: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    /// For source="file": the `accept` attribute (e.g. "image/*", "video/*").
    #[serde(default)]
    pub accept: String,
}
```

Add `runtime` to `ToolMeta` (after `format`):
```rust
    /// "number" or "text" or "image" or "video" — how to render the result.
    pub format: String,
    /// "wasm" (pure compute, default) or "ffmpeg" (file in → media out via browser ffmpeg).
    #[serde(default = "default_runtime")]
    pub runtime: String,
    #[serde(default, rename = "input")]
    pub inputs: Vec<Input>,
}

fn default_runtime() -> String {
    "wasm".to_string()
}
```

In `client_config()`, add `accept` per input and `runtime`/`format` at top level:
```rust
        let inputs: Vec<serde_json::Value> = self
            .inputs
            .iter()
            .map(|i| {
                serde_json::json!({
                    "name": i.name,
                    "source": i.source,
                    "accept": i.accept,
                    "elementId": format!("in-{}", i.name),
                })
            })
            .collect();
        serde_json::json!({
            "module": format!("./{}.js", self.wasm),
            "export": self.export,
            "runtime": self.runtime,
            "live": self.live,
            "intervalMs": self.interval_ms,
            "inputs": inputs,
            "output": { "label": self.output_label, "elementId": "tool-output" },
            "format": self.format,
        })
```

- [ ] **Step 4: Run to verify pass** — `cd gizza-ai && cargo test -p gizza-tool-pages`
Expected: PASS (all meta tests, including the existing ones — `runtime` is additive).

- [ ] **Step 5: Commit** — `git add tools/generator/src/meta.rs && git commit -m "feat(generator): meta runtime + file-input accept for media tool pages"`

### Task 2: `template.rs` — render file inputs + media output

**Files:** Modify `tools/generator/src/template.rs`

- [ ] **Step 1: Add the failing test** (append to `template.rs` `mod tests`)

```rust
    fn ffmpeg_sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-resize"
title         = "Resize"
description   = "d"
h1            = "Resize an image"
hero_subtitle = "s"
wasm          = "gizza_ai_image_resize_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Resized image"
format        = "image"

[[input]]
name   = "image"
source = "file"
accept = "image/*"
label  = "Image"

[[input]]
name   = "width"
source = "field"
label  = "Width (px)"
placeholder = "640"
"#,
        )
        .unwrap()
    }

    #[test]
    fn renders_file_input_and_media_output() {
        let html = render_page(&ffmpeg_sample(), "<h2>About</h2>");
        assert!(html.contains(r#"type="file""#), "file input present");
        assert!(html.contains(r#"id="in-image""#), "file input id");
        assert!(html.contains(r#"accept="image/*""#), "accept attr");
        assert!(html.contains(r#"id="in-width""#), "field input still present");
        // media output: an <img> result holder + a download link + a status output
        assert!(html.contains(r#"id="tool-output-media""#), "media output element");
        assert!(html.contains(r#"id="tool-output-download""#), "download link");
        assert!(html.contains(r#"id="tool-output""#), "status output for errors");
    }
```

- [ ] **Step 2: Run to verify failure** — `cd gizza-ai && cargo test -p gizza-tool-pages renders_file_input_and_media_output`
Expected: FAIL.

- [ ] **Step 3: Implement** — in `template.rs`, replace the `div class="tool-widget"` block's input loop + output with media-aware rendering:

```rust
                        div class="tool-widget" {
                            @for input in &meta.inputs {
                                @if input.source == "field" {
                                    label class="tool-field-label" for=(format!("in-{}", input.name)) { (input.label) }
                                    input id=(format!("in-{}", input.name)) class="tool-input"
                                          type="text" placeholder=(input.placeholder)
                                          autocomplete="off" autocapitalize="off" spellcheck="false";
                                } @else if input.source == "file" {
                                    label class="tool-field-label" for=(format!("in-{}", input.name)) { (input.label) }
                                    input id=(format!("in-{}", input.name)) class="tool-file"
                                          type="file" accept=(input.accept);
                                }
                            }
                            div class="tool-output-label" { (meta.output_label) }
                            @if meta.format == "image" || meta.format == "video" {
                                @if meta.format == "image" {
                                    img id="tool-output-media" class="tool-output-media" alt="" hidden;
                                } @else {
                                    video id="tool-output-media" class="tool-output-media" controls hidden {}
                                }
                                a id="tool-output-download" class="tool-output-download" download hidden { "Download" }
                                output id="tool-output" class="tool-output" { "" }
                            } @else {
                                output id="tool-output" class="tool-output" { "" }
                            }
                        }
```

(Note: for `format="video"` maud needs `video { }` with a body; the empty-body form `video ... {}` is shown. The pure-compute path — `format` "number"/"text" — keeps the single `<output>` exactly as before.)

- [ ] **Step 4: Run to verify pass** — `cd gizza-ai && cargo test -p gizza-tool-pages`
Expected: PASS (new test + the existing `includes_seo_head_and_widget` still passes — it uses `format="number"`, so it hits the `@else` single-output branch).

- [ ] **Step 5: Commit** — `git add tools/generator/src/template.rs && git commit -m "feat(generator): render file inputs + media output on tool pages"`

### Task 3: `main.rs` — copy `js/ffmpeg.js` for ffmpeg tools

**Files:** Modify `tools/generator/src/main.rs`

- [ ] **Step 1: Implement** — in `main.rs`, in the per-tool loop after the `tool.css` copy, add:

```rust
        copy_file(&root.join("site/tool.js"), &out.join("tool.js"))?;
        copy_file(&root.join("site/tool.css"), &out.join("tool.css"))?;
        if m.runtime == "ffmpeg" {
            copy_file(&root.join("js/ffmpeg.js"), &out.join("ffmpeg.js"))?;
        }
        eprintln!("rendered tools/{}/", m.slug);
```

- [ ] **Step 2: Verify it compiles** — `cd gizza-ai && cargo build -p gizza-tool-pages`
Expected: SUCCESS. (No unit test — this is a file copy; it's exercised end-to-end in Task 8.)

- [ ] **Step 3: Commit** — `git add tools/generator/src/main.rs && git commit -m "feat(generator): bundle js/ffmpeg.js into ffmpeg tool pages"`

### Task 4: `tool.js` ffmpeg branch + `tool.css` media styles

**Files:** Modify `site/tool.js`, `site/tool.css`; Create `js/tool-ffmpeg.test.js`

- [ ] **Step 1: Write failing unit tests for the pure helpers** (`js/tool-ffmpeg.test.js`)

Use the repo's existing JS test convention (check `js/render.test.js` for the runner — likely `node --test`). Test the two pure helpers we will extract:

```js
import { test } from "node:test";
import assert from "node:assert/strict";
import { inputNameFor, dataUrlFor } from "../site/tool-ffmpeg.js";

test("inputNameFor derives in.<ext> from a filename", () => {
  assert.equal(inputNameFor("cat.PNG"), "in.png");
  assert.equal(inputNameFor("clip.mp4"), "in.mp4");
  assert.equal(inputNameFor("noext"), "in.bin");
});

test("dataUrlFor builds a base64 data URL", () => {
  assert.equal(dataUrlFor("image/png", "AAAA"), "data:image/png;base64,AAAA");
});
```

- [ ] **Step 2: Run to verify failure** — `cd gizza-ai && node --test js/tool-ffmpeg.test.js`
Expected: FAIL (module `site/tool-ffmpeg.js` not found).

- [ ] **Step 3: Create `site/tool-ffmpeg.js`** with the pure helpers + the ffmpeg run flow:

```js
// ffmpeg tool-page helpers + run flow. Pure helpers are unit-tested; runFfmpeg
// is wired by tool.js for runtime === "ffmpeg" tools.

export function inputNameFor(filename) {
  const dot = filename.lastIndexOf(".");
  const ext = dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "bin";
  return `in.${ext || "bin"}`;
}

export function dataUrlFor(mime, b64) {
  return `data:${mime};base64,${b64}`;
}

function bytesToB64(u8) {
  let s = "";
  const chunk = 0x8000;
  for (let i = 0; i < u8.length; i += chunk) {
    s += String.fromCharCode.apply(null, u8.subarray(i, i + chunk));
  }
  return btoa(s);
}

// cfg: window.GIZZA_TOOL; mod: the loaded web-wasm module; ffmpegExec: from ./ffmpeg.js.
// Returns {ok, dataUrl?, mime?, outName?, error?}.
export async function runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs) {
  const inName = inputNameFor(file.name);
  const buf = new Uint8Array(await file.arrayBuffer());
  const bytes_b64 = bytesToB64(buf);

  // The web wasm builds the argv (pure, shared with the chat block's core).
  // Signature: build_argv(...fieldArgs, inName) -> { argv: string[], out_name: string }.
  let plan;
  try {
    plan = mod[cfg.export](...fieldArgs, inName);
  } catch (e) {
    return { ok: false, error: typeof e === "string" ? e : e && e.message ? e.message : "invalid args" };
  }
  const resp = await ffmpegExec(
    JSON.stringify(plan.argv),
    JSON.stringify([{ name: inName, bytes_b64 }]),
    plan.out_name
  );
  if (resp.exit_code !== 0 || !resp.output_b64) {
    const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
    return { ok: false, error: snippet };
  }
  const mime = file.type || "application/octet-stream";
  return { ok: true, dataUrl: dataUrlFor(mime, resp.output_b64), mime, outName: plan.out_name };
}
```

- [ ] **Step 4: Run to verify the helper tests pass** — `cd gizza-ai && node --test js/tool-ffmpeg.test.js`
Expected: PASS.

- [ ] **Step 5: Wire `tool.js` to branch on `cfg.runtime`** — in `site/tool.js`, at the top of `main()` after loading `mod`, add the ffmpeg path; leave the existing pure path as the `else`:

```js
async function main() {
  let mod;
  try {
    mod = await import(cfg.module);
    await mod.default();
  } catch (e) {
    showError("Failed to load tool.");
    return;
  }

  if (cfg.runtime === "ffmpeg") {
    const { runFfmpeg } = await import("./tool-ffmpeg.js");
    const { ffmpegExec } = await import("./ffmpeg.js");
    const media = document.getElementById("tool-output-media");
    const dl = document.getElementById("tool-output-download");
    const fileInput = document.getElementById(
      "in-" + (cfg.inputs.find((i) => i.source === "file") || {}).name
    );
    const fieldInputs = cfg.inputs.filter((i) => i.source === "field");

    async function run() {
      const file = fileInput && fileInput.files && fileInput.files[0];
      if (!file) return;
      out.textContent = "Processing…";
      out.classList.remove("error");
      media.hidden = true;
      dl.hidden = true;
      const fieldArgs = fieldInputs.map((i) => {
        const el = document.getElementById(i.elementId);
        return el ? el.value : "";
      });
      const r = await runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs);
      if (r.ok) {
        out.textContent = "";
        media.src = r.dataUrl;
        media.hidden = false;
        dl.href = r.dataUrl;
        dl.download = r.outName;
        dl.hidden = false;
      } else {
        showError(r.error);
      }
    }

    if (fileInput) fileInput.addEventListener("change", run);
    for (const i of fieldInputs) {
      const el = document.getElementById(i.elementId);
      if (el) el.addEventListener("input", run);
    }
    return;
  }

  // ── pure-compute path (unchanged) ──
  const fn = mod[cfg.export];
  // ...rest of the existing main() body...
}
```

(Keep the rest of the existing pure path verbatim below the `return;`.)

- [ ] **Step 6: Add media styles** to `site/tool.css`:

```css
.tool-file { display: block; width: 100%; margin-bottom: 10px; font-size: 14px; }
.tool-output-media { display: block; max-width: 100%; border-radius: 8px; margin-top: 4px; }
.tool-output-media[hidden] { display: none; }
.tool-output-download { display: inline-block; margin-top: 8px; font-size: 14px; font-weight: 600;
  color: var(--tool-accent); text-decoration: underline; }
.tool-output-download[hidden] { display: none; }
```

- [ ] **Step 7: Commit** — `git add site/tool.js site/tool-ffmpeg.js site/tool.css js/tool-ffmpeg.test.js && git commit -m "feat(tool-page): ffmpeg runtime branch (file → argv → ffmpegExec → media)"`

---

## Milestone B — give `image-resize` a page (the proof)

### Task 5: extract `image-resize` argv logic into `core/`

**Files:** Modify `blocks/image-resize/Cargo.toml`, `blocks/image-resize/src/lib.rs`; Create `blocks/image-resize/core/Cargo.toml`, `blocks/image-resize/core/src/lib.rs`

- [ ] **Step 1: Create `blocks/image-resize/core/Cargo.toml`**

```toml
[package]
name = "gizza-ai-image-resize-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[dependencies]
```

- [ ] **Step 2: Create `blocks/image-resize/core/src/lib.rs`** — move `Fit`, `parse_fit`, `build_argv`, plus a new `out_name` helper and a `plan_resize` that the web wrapper uses. Include the moved unit tests.

```rust
//! gizza-ai/image-resize core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Fit { Contain, Cover, Stretch }

pub fn parse_fit(s: Option<&str>) -> Result<Fit, String> {
    match s.unwrap_or("contain") {
        "contain" => Ok(Fit::Contain),
        "cover"   => Ok(Fit::Cover),
        "stretch" => Ok(Fit::Stretch),
        other     => Err(format!("invalid fit {other:?}; expected contain|cover|stretch")),
    }
}

/// Build the ffmpeg argv for a resize (no leading "ffmpeg").
pub fn build_argv(in_name: &str, out_name: &str, w: Option<u32>, h: Option<u32>, fit: Fit) -> Vec<String> {
    let (sw, sh) = (
        w.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
        h.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
    );
    let vf = match fit {
        Fit::Stretch => format!("scale={sw}:{sh}"),
        Fit::Contain => format!("scale={sw}:{sh}:force_original_aspect_ratio=decrease"),
        Fit::Cover   => format!("scale={sw}:{sh}:force_original_aspect_ratio=increase,crop={sw}:{sh}"),
    };
    vec!["-i".into(), in_name.into(), "-vf".into(), vf, out_name.into()]
}

/// Validate dimensions + fit and return `(argv, out_name)` for an input file.
/// `out_name` keeps the input extension. Used by the web page (file in → file out).
pub fn plan_resize(in_name: &str, w: Option<u32>, h: Option<u32>, fit: Fit) -> Result<(Vec<String>, String), String> {
    if w.is_none() && h.is_none() {
        return Err("at least one of width/height is required".into());
    }
    if w == Some(0) || h == Some(0) {
        return Err("width/height must be > 0".into());
    }
    if fit == Fit::Cover && (w.is_none() || h.is_none()) {
        return Err("fit=cover requires both width and height".into());
    }
    let ext = in_name.rsplit('.').next().filter(|e| !e.is_empty()).unwrap_or("png");
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, w, h, fit), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_contain_both_dims() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Contain);
        assert_eq!(argv, vec![
            "-i".to_string(), "in.png".to_string(), "-vf".to_string(),
            "scale=640:480:force_original_aspect_ratio=decrease".to_string(), "out.png".to_string(),
        ]);
    }

    #[test]
    fn parse_fit_default_is_contain() { assert_eq!(parse_fit(None).unwrap(), Fit::Contain); }

    #[test]
    fn parse_fit_rejects_unknown() { assert!(parse_fit(Some("squish")).is_err()); }

    #[test]
    fn plan_resize_keeps_extension_and_validates() {
        let (argv, out) = plan_resize("in.jpg", Some(320), None, Fit::Contain).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.iter().any(|a| a == "scale=320:-1:force_original_aspect_ratio=decrease"));
        assert!(plan_resize("in.png", None, None, Fit::Contain).is_err());
        assert!(plan_resize("in.png", Some(0), Some(10), Fit::Stretch).is_err());
        assert!(plan_resize("in.png", Some(10), None, Fit::Cover).is_err());
    }
}
```

- [ ] **Step 3: Update `blocks/image-resize/Cargo.toml`** — workspace members + add the core dep:

```toml
[workspace]
resolver = "2"
members = [".", "core", "web"]

[package]
name = "gizza-ai-image-resize-block"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wafer-sdk = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
wafer-block = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
gizza-ai-block-utils = { path = "../../block-utils" }
gizza-ai-image-resize-core = { path = "core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
```

- [ ] **Step 4: Update `blocks/image-resize/src/lib.rs`** — delete the moved `Fit`/`parse_fit`/`build_argv` + their `#[cfg(test)]` argv/parse_fit tests, and `use gizza_ai_image_resize_core::{build_argv, parse_fit, Fit};` instead. Keep `run()`, the `Args` struct, `summary()`, and the filename tests. (The `run()` body already calls `build_argv(&ffmpeg_in, &ffmpeg_out, args.width, args.height, fit)` and `parse_fit(args.fit.as_deref())` — those now resolve to the `core::` items via the `use`.)

- [ ] **Step 5: Verify the chat block + core build and test** —
Run: `cd gizza-ai && cargo test -p gizza-ai-image-resize-core` → PASS (moved tests).
Run: `cd gizza-ai && wafer build blocks/image-resize` → SUCCESS (the wasm32 chat block compiles using `core`).
Run: `cd gizza-ai && cargo test -p gizza-ai-image-resize-block` → PASS (the remaining native tests: `summary`/filename).
Expected: all green; the chat block's behavior is unchanged (pure move).

- [ ] **Step 6: Commit** — `git add blocks/image-resize && git commit -m "refactor(image-resize): extract pure argv logic into core (shared by chat + page)"`

### Task 6: `image-resize` `web/` — wasm-bindgen `build_argv`

**Files:** Create `blocks/image-resize/web/Cargo.toml`, `blocks/image-resize/web/src/lib.rs`

- [ ] **Step 1: Create `blocks/image-resize/web/Cargo.toml`**

```toml
[package]
name = "gizza-ai-image-resize-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
gizza-ai-image-resize-core = { path = "../core" }
```

- [ ] **Step 2: Create `blocks/image-resize/web/src/lib.rs`** — the page calls `build_argv(width, height, fit, in_name)`; `width`/`height` are `f64` (0 = unset — avoids the i64→BigInt gotcha), `fit` a string, returns `{argv, out_name}`:

```rust
//! Browser-facing wasm-bindgen wrapper for the standalone /tools/image-resize/ page.
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use gizza_ai_image_resize_core::{parse_fit, plan_resize};

#[derive(Serialize)]
struct Plan {
    argv: Vec<String>,
    out_name: String,
}

/// `width`/`height` of 0 mean "unset" (auto). Returns `{ argv: string[], out_name }`
/// or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(width: f64, height: f64, fit: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let w = if width > 0.0 { Some(width as u32) } else { None };
    let h = if height > 0.0 { Some(height as u32) } else { None };
    let fit = parse_fit(Some(fit)).map_err(|e| JsValue::from_str(&e))?;
    let (argv, out_name) = plan_resize(in_name, w, h, fit).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
```

- [ ] **Step 3: Build the web wasm** —
Run: `cd gizza-ai && wasm-pack build blocks/image-resize/web --target web --release --out-dir pkg`
Expected: SUCCESS; produces `blocks/image-resize/web/pkg/gizza_ai_image_resize_web.js` + `_bg.wasm`.

- [ ] **Step 4: Commit** — `git add blocks/image-resize/web && git commit -m "feat(image-resize): web/ build_argv export for the standalone page"`

### Task 7: `image-resize` `page/` — meta + content

**Files:** Create `blocks/image-resize/page/meta.toml`, `blocks/image-resize/page/content.md`

- [ ] **Step 1: Create `blocks/image-resize/page/meta.toml`**

```toml
slug          = "image-resize"
title         = "Resize an Image Online — gizza.ai"
description   = "Resize any image right in your browser — set a width and/or height, pick a fit mode. No upload to a server, runs locally, free."
tags          = ["image", "resize", "scale", "thumbnail", "dimensions"]
h1            = "Resize an Image"
hero_subtitle = "Pick an image, set a size — it's resized in your browser, nothing is uploaded."
wasm          = "gizza_ai_image_resize_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Resized image"
format        = "image"

[[input]]
name   = "image"
source = "file"
accept = "image/*"
label  = "Image"

[[input]]
name        = "width"
source      = "field"
label       = "Width (px)"
placeholder = "640"

[[input]]
name        = "height"
source      = "field"
label       = "Height (px)"
placeholder = "(optional)"

[[input]]
name        = "fit"
source      = "field"
label       = "Fit (contain|cover|stretch)"
placeholder = "contain"
```

> Note the input ORDER matters: `tool.js` passes the `field` inputs to `build_argv(...)` in declared order, i.e. `build_argv(width, height, fit, in_name)`. Width, height, fit, then the file name is appended by `runFfmpeg`. The `web/src/lib.rs` signature matches: `build_argv(width: f64, height: f64, fit: &str, in_name: &str)`. Empty field → `""`; `runFfmpeg` passes the raw strings, but `build_argv` expects f64 for width/height — so **the field values must be coerced to numbers in `runFfmpeg`** (wasm-bindgen marshals JS strings/numbers to `f64` only from numbers). Handle in Task 8 verification: `tool.js` field args are strings; for the ffmpeg path, coerce numeric-looking field args to Number before the call. Add to `runFfmpeg` (Task 4 file) a coercion: `const a = fieldArgs.map(v => v === "" ? 0 : (isNaN(Number(v)) ? v : Number(v)))` and pass `a`. (If you prefer, make `build_argv` take strings and parse — but numbers keep the wasm signature clean. Pick one and keep `meta` input order = wasm param order.)

- [ ] **Step 2: Create `blocks/image-resize/page/content.md`**

```markdown
## Resize an image in your browser

Pick an image, type a width (and optionally a height), and get a resized copy
instantly. The resizing runs entirely in your browser with ffmpeg compiled to
WebAssembly — your image is never uploaded to a server.

### Fit modes

- **contain** (default) — fit inside the box, keep aspect ratio.
- **cover** — fill the box, keep aspect ratio, crop the overflow (needs both width and height).
- **stretch** — force the exact width × height, ignoring aspect ratio.

### Tips

- Give only a width (or only a height) to scale proportionally.
- Works offline once the page has loaded.
```

- [ ] **Step 3: Commit** — `git add blocks/image-resize/page && git commit -m "feat(image-resize): standalone page meta + content"`

### Task 8: build, generate, and Playwright-test the page

**Files:** Create `tests/fixtures/red-2x2.png`, `tests/tool-page-image-resize.spec.ts` (match the repo's Playwright convention under `tests/`)

- [ ] **Step 1: Reconcile the field-arg coercion** (from Task 7's note) — ensure `runFfmpeg` (in `site/tool-ffmpeg.js`) coerces numeric field args before calling `build_argv`. Update the `fieldArgs` it receives by mapping in `tool.js`'s ffmpeg `run()`:

```js
      const fieldArgs = fieldInputs.map((i) => {
        const el = document.getElementById(i.elementId);
        const v = el ? el.value : "";
        return v === "" ? 0 : isNaN(Number(v)) ? v : Number(v);
      });
```

(So `width="640"` → `640` (Number, marshals to f64), `fit="contain"` → stays a string. Empty width → `0` → core treats as unset.)

- [ ] **Step 2: Add a tiny fixture image** — create `tests/fixtures/red-2x2.png` (a 2×2 red PNG):

```bash
cd /home/joris/Programs/suppers-ai/workspace/gizza-ai
mkdir -p tests/fixtures
python3 - <<'PY'
import struct, zlib, pathlib
def png(width, height, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    idat = zlib.compress(raw)
    pathlib.Path("tests/fixtures/red-2x2.png").write_bytes(
        sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b""))
PY
ls -la tests/fixtures/red-2x2.png
```

- [ ] **Step 3: Full build + generate** —
```bash
cd /home/joris/Programs/suppers-ai/workspace/gizza-ai
wafer build blocks/image-resize
wasm-pack build blocks/image-resize/web --target web --release --out-dir pkg
solobase build                 # rebuilds app + all blocks
cargo run --manifest-path tools/generator/Cargo.toml -- .
ls -la pkg/tools/image-resize/index.html pkg/tools/image-resize/ffmpeg.js pkg/tools/image-resize/gizza_ai_image_resize_web_bg.wasm
```
Expected: all four files present (the page, the bundled ffmpeg.js, the web wasm + js).

- [ ] **Step 4: Write the Playwright test** (`tests/tool-page-image-resize.spec.ts`, matching the existing `tests/` Playwright setup — check `tests/package.json`/existing spec for the import + config style):

```ts
import { test, expect } from "@playwright/test";
import * as path from "path";

// Serve pkg/ statically before running (see Step 5). BASE points at it.
const BASE = process.env.TOOL_BASE || "http://localhost:8011";

test("image-resize page resizes an uploaded image", async ({ page }) => {
  await page.goto(`${BASE}/tools/image-resize/`);
  await page.waitForSelector("#in-image");

  await page.fill("#in-width", "1");
  await page.setInputFiles("#in-image", path.resolve(__dirname, "fixtures/red-2x2.png"));

  // ffmpeg loads from CDN on first run; allow generous time, then assert the
  // output <img> got a data URL.
  const media = page.locator("#tool-output-media");
  await expect(media).toBeVisible({ timeout: 60_000 });
  const src = await media.getAttribute("src");
  expect(src).toMatch(/^data:image\//);
});
```

- [ ] **Step 5: Run it** —
```bash
cd /home/joris/Programs/suppers-ai/workspace/gizza-ai
python3 -m http.server --directory pkg 8011 &   # serve the built pages
SERVER=$!
cd tests && TOOL_BASE=http://localhost:8011 npx playwright test tool-page-image-resize.spec.ts
RESULT=$?
kill $SERVER
[ $RESULT -eq 0 ] && echo "PASS"
```
Expected: PASS — the uploaded 2×2 PNG resized to width 1 yields a `data:image/png` result in `#tool-output-media`. (The test needs network for the @ffmpeg CDN; if the CI/sandbox blocks it, mark the test `@network` and document that it runs where the CDN is reachable.)

- [ ] **Step 6: CLI sanity (unchanged behavior)** — confirm the chat/CLI path still works after the core refactor:
```bash
cargo install --path cli --force
gizza tool image-resize url=https://example.com/nope.png width=8   # structured error (no panic), not a crash
```
Expected: a clean error (the refactor didn't change the chat path).

- [ ] **Step 7: Commit** — `git add tests/fixtures/red-2x2.png tests/tool-page-image-resize.spec.ts site/tool-ffmpeg.js site/tool.js && git commit -m "test(tool-page): Playwright e2e for the image-resize media page + field coercion"`

---

## Self-review notes

- **Spec coverage:** §Phase 0 change-set items 1–5 map to Tasks 1–4 (generator + tool.js) and Tasks 5–8 (image-resize page = the proof). The "argv-in-core single source" → Task 5; "reuse ffmpegExec, header-free" → Tasks 3–4; "Playwright upload→output" → Task 8.
- **Known compile-time confirmations (flagged inline):** the generator package name for `-p` (Task 1 Step 2 — use the name in `tools/generator/Cargo.toml`); the repo's JS test runner + Playwright `tests/` config style (Tasks 4, 8 — mirror existing `js/*.test.js` and `tests/`); `serde-wasm-bindgen` version (Task 6 — pin to whatever the wasm-pack toolchain resolves). Each has a concrete check, not a placeholder.
- **Field-arg order is load-bearing:** `meta.toml` field-input order (width, height, fit) MUST match `web build_argv(width, height, fit, in_name)`. Called out in Task 7 + enforced by the Task 8 test.
- **Network dependency:** the Playwright test pulls `@ffmpeg/core` from jsDelivr; if the runner has no network, the test self-documents as `@network`. The unit tests (Tasks 1,2,4,5) and the build (Tasks 3,6,8 build steps) are offline.
- **Follow-up (Phase 1, separate plan):** the `new-tool` skill that clones this `image-resize` shape for arbitrary tools.
```
