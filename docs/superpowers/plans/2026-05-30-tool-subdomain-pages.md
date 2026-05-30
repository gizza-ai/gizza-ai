# Per-tool Standalone Subdomain Pages — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each pure-compute gizza-ai tool a standalone static page on its own `*.gizza.ai` subdomain, sharing compute logic with the chat skill via a pure Rust core, deployed on Cloudflare Pages.

**Architecture:** Per tool, a pure `core` rlib crate holds the logic; the existing chat skill block and a new `wasm-bindgen` `web` cdylib both depend on it (single source of truth). A standalone Rust generator renders a shared `maud` template + per-tool `meta.toml`/`content.md` into `pkg/tools/<tool>/index.html` with SEO head tags and JSON-LD. A Cloudflare Pages Function rewrites `<sub>.gizza.ai` → `/tools/<sub>/`.

**Tech Stack:** Rust (`meval`, `wasm-bindgen`, `maud`, `pulldown-cmark`, `toml`/`serde`), `wasm-pack`, Cloudflare Pages + Functions, Playwright, Node test runner.

---

## File Structure

**Calculator vertical slice**
- Create `blocks/calculator/core/Cargo.toml` — `gizza-ai-calculator-core` rlib, dep `meval`.
- Create `blocks/calculator/core/src/lib.rs` — `pub fn evaluate(&str) -> Result<f64,String>` + tests.
- Modify `blocks/calculator/Cargo.toml` — add path dep on `core`.
- Modify `blocks/calculator/src/lib.rs` — `handle()` delegates to `core::evaluate`.
- Create `blocks/calculator/web/Cargo.toml` — `gizza-ai-calculator-web` cdylib, deps `wasm-bindgen` + `core`.
- Create `blocks/calculator/web/src/lib.rs` — `#[wasm_bindgen] pub fn evaluate(...)`.
- Create `blocks/calculator/page/meta.toml`, `blocks/calculator/page/content.md`.

**Clock second tool** (mirrors calculator)
- Create `blocks/clock/core/{Cargo.toml,src/lib.rs}` — `pub fn format_iso8601(i64) -> String`.
- Modify `blocks/clock/{Cargo.toml,src/lib.rs}` — depend on + delegate to core.
- Create `blocks/clock/web/{Cargo.toml,src/lib.rs}` — `#[wasm_bindgen] pub fn format_time(i64) -> String`.
- Create `blocks/clock/page/{meta.toml,content.md}`.

**Generator** (own workspace, not part of gizza-ai crate graph)
- Create `tools/generator/Cargo.toml` — bin `gizza-tool-pages`, deps `maud serde toml pulldown-cmark serde_json`.
- Create `tools/generator/src/{main.rs,meta.rs,template.rs,seo.rs}`.

**Shared page runtime**
- Create `site/tool.js` — generic driver reading `window.GIZZA_TOOL`.
- Create `site/tool.css` — option-C styling.

**Routing + deploy**
- Create `functions/routing.mjs` — pure `resolve(host,pathname)` mapping.
- Create `functions/routing.test.mjs` — node test.
- Create `functions/_middleware.js` — Cloudflare middleware using `resolve`.
- Modify `.github/workflows/deploy.yml` — build tool pages + `wrangler pages deploy`.
- Modify `justfile` — `build-tools` recipe; `serve` depends on it.
- Modify `build.rs` — scan `blocks/*/page/meta.toml` → generate `TOOLS` const.
- Modify `src/blocks/ui.rs` — render "Tools" section from `TOOLS`.
- Remove `static/CNAME` + its `solobase.toml` overlay.
- Create `tests/tool_pages.spec.ts` — Playwright page test.

**Conventions for every task:** run cargo commands from the relevant crate dir; the worktree root is `/home/joris/Programs/suppers-ai/workspace/gizza-ai/.claude/worktrees/tool-subdomain-pages` (referred to below as `$ROOT`).

---

## Task 1: Calculator core crate

**Files:**
- Create: `blocks/calculator/core/Cargo.toml`
- Create: `blocks/calculator/core/src/lib.rs`

- [ ] **Step 1: Write the failing test** — `blocks/calculator/core/src/lib.rs`

```rust
//! gizza-ai/calculator core — pure arithmetic evaluation shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.

/// Evaluate an arithmetic expression. Returns `Err` if meval fails to parse it,
/// or if the result is non-finite (NaN/Inf) — typically division by zero or
/// overflow.
pub fn evaluate(expr: &str) -> Result<f64, String> {
    let v = meval::eval_str(expr).map_err(|e| format!("eval failed: {e}"))?;
    if !v.is_finite() {
        return Err(format!("eval failed: non-finite result ({v})"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_simple_arithmetic() {
        assert_eq!(evaluate("2+2").unwrap(), 4.0);
        assert_eq!(evaluate("2+2*3").unwrap(), 8.0);
    }

    #[test]
    fn evaluates_named_functions() {
        assert_eq!(evaluate("sqrt(16)").unwrap(), 4.0);
        let v = evaluate("3.14 * 2^2").unwrap();
        assert!((v - 12.56).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn rejects_non_finite_results() {
        let err = evaluate("1/0").unwrap_err();
        assert!(err.contains("non-finite"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_syntax() {
        let err = evaluate("nonsense === ===").unwrap_err();
        assert!(err.contains("eval failed"), "got: {err}");
    }
}
```

- [ ] **Step 2: Create the manifest** — `blocks/calculator/core/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["."]

[package]
name = "gizza-ai-calculator-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[dependencies]
meval = "0.2"
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cd $ROOT/blocks/calculator/core && cargo test`
Expected: PASS, 4 tests.

- [ ] **Step 4: Commit**

```bash
cd $ROOT
git add blocks/calculator/core
git commit -m "feat(calculator): extract pure core crate with evaluate()"
```

---

## Task 2: Calculator block delegates to core

**Files:**
- Modify: `blocks/calculator/Cargo.toml`
- Modify: `blocks/calculator/src/lib.rs`

- [ ] **Step 1: Add core dep** — append to `[dependencies]` in `blocks/calculator/Cargo.toml`

```toml
gizza-ai-calculator-core = { path = "core" }
```

- [ ] **Step 2: Replace logic with delegation** — `blocks/calculator/src/lib.rs`

Replace the whole file with:

```rust
//! gizza-ai/calculator — evaluates math expressions.
//!
//! Thin chat-skill wrapper around `gizza-ai-calculator-core`. Takes
//! `{ "expr": "..." }`, returns `{ "result": <number> }` or `{ "error": "..." }`.
//! No host calls — runs entirely inside the WASM sandbox.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expr: String,
}

#[cfg(target_arch = "wasm32")]
struct Calculator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Calculator skill",
    skill(
        description = "Evaluate an arithmetic expression (e.g. '2+2*3'). Returns the numeric result.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "expr": { "type": "string", "description": "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2')." }
            },
            "required": ["expr"],
            "additionalProperties": false
        }"#
    ),
)]
impl Calculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match gizza_ai_calculator_core::evaluate(&args.expr) {
            Ok(v) => {
                let body = serde_json::json!({ "result": v });
                GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
            }
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    let body = serde_json::json!({ "error": msg });
    GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
}
```

- [ ] **Step 3: Verify the block still compiles (native)**

Run: `cd $ROOT/blocks/calculator && cargo build`
Expected: builds clean. (The `#[wafer_block]` impl is wasm-gated; native build just checks the non-gated code + dep wiring.)

- [ ] **Step 4: Commit**

```bash
cd $ROOT
git add blocks/calculator/Cargo.toml blocks/calculator/src/lib.rs
git commit -m "refactor(calculator): delegate block logic to core crate"
```

---

## Task 3: Calculator web (wasm-bindgen) wrapper

**Files:**
- Create: `blocks/calculator/web/Cargo.toml`
- Create: `blocks/calculator/web/src/lib.rs`

- [ ] **Step 1: Write the wrapper** — `blocks/calculator/web/src/lib.rs`

```rust
//! Browser-facing wasm-bindgen wrapper around `gizza-ai-calculator-core`.
//! Compiled with wasm-pack for the standalone calculator.gizza.ai page.

use wasm_bindgen::prelude::*;

/// Evaluate an arithmetic expression. Throws a JS error string on failure.
#[wasm_bindgen]
pub fn evaluate(expr: &str) -> Result<f64, JsValue> {
    gizza_ai_calculator_core::evaluate(expr).map_err(|e| JsValue::from_str(&e))
}
```

- [ ] **Step 2: Create the manifest** — `blocks/calculator/web/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["."]

[package]
name = "gizza-ai-calculator-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
gizza-ai-calculator-core = { path = "../core" }
```

- [ ] **Step 3: Build with wasm-pack to verify**

Run: `cd $ROOT/blocks/calculator/web && wasm-pack build --target web --release --out-dir pkg`
Expected: produces `pkg/gizza_ai_calculator_web.js` + `pkg/gizza_ai_calculator_web_bg.wasm` (small, tens of KB).

- [ ] **Step 4: Confirm the wasm is small** (sanity, no runtime bloat)

Run: `ls -la $ROOT/blocks/calculator/web/pkg/*_bg.wasm`
Expected: file present, well under 200 KB.

- [ ] **Step 5: Ignore the per-tool pkg output** — append to `$ROOT/.gitignore`

```
# per-tool web wasm build output
blocks/*/web/pkg/
blocks/*/web/target/
```

- [ ] **Step 6: Commit**

```bash
cd $ROOT
git add blocks/calculator/web/Cargo.toml blocks/calculator/web/src/lib.rs .gitignore
git commit -m "feat(calculator): wasm-bindgen web wrapper for standalone page"
```

---

## Task 4: Calculator page metadata + content

**Files:**
- Create: `blocks/calculator/page/meta.toml`
- Create: `blocks/calculator/page/content.md`

- [ ] **Step 1: Write the metadata** — `blocks/calculator/page/meta.toml`

```toml
subdomain     = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate any arithmetic expression instantly in your browser. Supports +, −, ×, ÷, parentheses and functions. No sign-up, runs offline."
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression and get the answer instantly — runs entirely in your browser."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
```

- [ ] **Step 2: Write the SEO content** — `blocks/calculator/page/content.md`

```markdown
## About this calculator

This free online calculator evaluates arithmetic expressions instantly, right
in your browser. Nothing is sent to a server — the math runs locally, works
offline, and needs no sign-up.

### Supported operations

- Add, subtract, multiply, divide: `+`, `-`, `*`, `/`
- Parentheses for grouping: `(1 + 2) * 3`
- Powers with `^`: `2^10`
- Functions: `sqrt`, `sin`, `cos`, `tan`, `ln`, `abs`, and more

### Examples

- `2 + 2 * 3` → `8`
- `sqrt(16)` → `4`
- `3.14 * 2^2` → `12.56`

### FAQ

**Is it really free?** Yes — and private. Your input never leaves your device.

**Does it work offline?** Yes, once the page has loaded.
```

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add blocks/calculator/page
git commit -m "feat(calculator): page metadata and SEO content"
```

---

## Task 5: Generator — metadata model

**Files:**
- Create: `tools/generator/Cargo.toml`
- Create: `tools/generator/src/meta.rs`
- Create: `tools/generator/src/main.rs` (stub so the crate builds + test runs)

- [ ] **Step 1: Write the failing test** — bottom of `tools/generator/src/meta.rs`

```rust
//! Per-tool page metadata, parsed from `blocks/<tool>/page/meta.toml`.

use serde::Deserialize;

/// One input field for a tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    /// "field" = a visible text input; "clock" = current unix seconds, supplied by JS.
    pub source: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
}

/// Full metadata for a single tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ToolMeta {
    pub subdomain: String,
    pub title: String,
    pub description: String,
    pub h1: String,
    pub hero_subtitle: String,
    /// wasm-pack output basename (without extension), e.g. "gizza_ai_calculator_web".
    pub wasm: String,
    /// Exported function name to call.
    pub export: String,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub interval_ms: Option<u64>,
    pub output_label: String,
    /// "number" or "text" — how to render the result.
    pub format: String,
    #[serde(default, rename = "input")]
    pub inputs: Vec<Input>,
}

impl ToolMeta {
    /// Parse from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("meta.toml parse error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calculator_meta() {
        let text = r#"
subdomain     = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "desc"
h1            = "Free Online Calculator"
hero_subtitle = "sub"
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.subdomain, "calculator");
        assert_eq!(m.export, "evaluate");
        assert_eq!(m.inputs.len(), 1);
        assert_eq!(m.inputs[0].source, "field");
        assert!(!m.live);
    }

    #[test]
    fn parses_live_tool_without_inputs_fields() {
        let text = r#"
subdomain     = "clock"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "gizza_ai_clock_web"
export        = "format_time"
live          = true
interval_ms   = 1000
output_label  = "Current time (UTC)"
format        = "text"

[[input]]
name   = "unix"
source = "clock"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert!(m.live);
        assert_eq!(m.interval_ms, Some(1000));
        assert_eq!(m.inputs[0].source, "clock");
    }
}
```

- [ ] **Step 2: Create manifest** — `tools/generator/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["."]

[package]
name = "gizza-tool-pages"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[[bin]]
name = "gizza-tool-pages"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
maud = "0.27"
pulldown-cmark = "0.12"
```

- [ ] **Step 3: Create a minimal main.rs so the crate builds** — `tools/generator/src/main.rs`

```rust
mod meta;
mod template;
mod seo;

fn main() {
    if let Err(e) = run() {
        eprintln!("gizza-tool-pages: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    // Filled in by Task 8.
    Ok(())
}
```

- [ ] **Step 4: Create empty module files so `mod` resolves** — `tools/generator/src/template.rs` and `tools/generator/src/seo.rs`

`template.rs`:
```rust
//! Page template rendering (Task 7).
```
`seo.rs`:
```rust
//! Sitemap / robots / JSON-LD helpers (Task 9).
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd $ROOT/tools/generator && cargo test meta::`
Expected: PASS, 2 tests.

- [ ] **Step 6: Commit**

```bash
cd $ROOT
git add tools/generator
git commit -m "feat(generator): ToolMeta model + scaffold"
```

---

## Task 6: Generator — the shared client config (window.GIZZA_TOOL)

**Files:**
- Modify: `tools/generator/src/meta.rs`

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `tools/generator/src/meta.rs`

```rust
    #[test]
    fn builds_client_config_json() {
        let text = r#"
subdomain     = "calculator"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        let cfg = m.client_config();
        assert_eq!(cfg["module"], "./gizza_ai_calculator_web.js");
        assert_eq!(cfg["export"], "evaluate");
        assert_eq!(cfg["live"], false);
        assert_eq!(cfg["inputs"][0]["source"], "field");
        assert_eq!(cfg["output"]["label"], "Result");
        assert_eq!(cfg["format"], "number");
    }
```

- [ ] **Step 2: Implement `client_config`** — add to `impl ToolMeta` in `tools/generator/src/meta.rs`

```rust
    /// Build the `window.GIZZA_TOOL` config object consumed by `tool.js`.
    pub fn client_config(&self) -> serde_json::Value {
        let inputs: Vec<serde_json::Value> = self
            .inputs
            .iter()
            .map(|i| {
                serde_json::json!({
                    "name": i.name,
                    "source": i.source,
                    "elementId": format!("in-{}", i.name),
                })
            })
            .collect();
        serde_json::json!({
            "module": format!("./{}.js", self.wasm),
            "export": self.export,
            "live": self.live,
            "intervalMs": self.interval_ms,
            "inputs": inputs,
            "output": { "label": self.output_label, "elementId": "tool-output" },
            "format": self.format,
        })
    }
```

Add `use serde_json;` is not needed (crate dep). Ensure `serde_json` is referenced via full path as above.

- [ ] **Step 3: Run the test**

Run: `cd $ROOT/tools/generator && cargo test meta::`
Expected: PASS, 3 tests.

- [ ] **Step 4: Commit**

```bash
cd $ROOT
git add tools/generator/src/meta.rs
git commit -m "feat(generator): client_config() for window.GIZZA_TOOL"
```

---

## Task 7: Generator — page template (option C)

**Files:**
- Modify: `tools/generator/src/template.rs`

- [ ] **Step 1: Write the failing test** — `tools/generator/src/template.rs`

```rust
//! Renders the option-C tool page: top nav + hero tool + SEO content + footer,
//! with SEO `<head>` tags and JSON-LD.

use crate::meta::ToolMeta;
use maud::{html, PreEscaped, DOCTYPE};

/// Render the full HTML document for a tool page. `content_html` is the
/// markdown-rendered SEO section.
pub fn render_page(meta: &ToolMeta, content_html: &str) -> String {
    let canonical = format!("https://{}.gizza.ai/", meta.subdomain);
    let client_cfg = meta.client_config().to_string();
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebApplication",
        "name": meta.h1,
        "description": meta.description,
        "url": canonical,
        "applicationCategory": "UtilitiesApplication",
        "operatingSystem": "Any",
        "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" },
        "publisher": { "@type": "Organization", "name": "gizza.ai", "url": "https://gizza.ai" }
    })
    .to_string();

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (meta.title) }
                meta name="description" content=(meta.description);
                link rel="canonical" href=(canonical);
                meta property="og:type" content="website";
                meta property="og:title" content=(meta.title);
                meta property="og:description" content=(meta.description);
                meta property="og:url" content=(canonical);
                meta property="og:image" content="https://gizza.ai/gis.png";
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(meta.title);
                meta name="twitter:description" content=(meta.description);
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                link rel="stylesheet" href="./tool.css";
                link rel="icon" href="https://gizza.ai/favicon-32.png" sizes="32x32";
                script type="application/ld+json" { (PreEscaped(json_ld)) }
            }
            body {
                header class="tool-nav" {
                    a class="tool-brand" href="https://gizza.ai" {
                        img src="https://gizza.ai/gis_no_eyes.png" alt="gizza.ai logo";
                        span { "gizza.ai" }
                    }
                    a class="tool-chat-link" href="https://gizza.ai" { "Open AI chat →" }
                }
                main class="tool-main" {
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                        div class="tool-widget" {
                            @for input in &meta.inputs {
                                @if input.source == "field" {
                                    label class="tool-field-label" for=(format!("in-{}", input.name)) { (input.label) }
                                    input id=(format!("in-{}", input.name)) class="tool-input"
                                          type="text" placeholder=(input.placeholder)
                                          autocomplete="off" autocapitalize="off" spellcheck="false";
                                }
                            }
                            div class="tool-output-label" { (meta.output_label) }
                            output id="tool-output" class="tool-output" { "" }
                        }
                    }
                    section class="tool-content" {
                        (PreEscaped(content_html))
                    }
                }
                footer class="tool-footer" {
                    div class="tool-footer-brand" {
                        img src="https://gizza.ai/gis_no_eyes.png" alt="";
                        span { "⚡ Powered by gizza.ai" }
                    }
                    p {
                        strong { "gizza.ai" }
                        " is a free, private AI assistant that runs entirely in your browser — no server, no sign-up, your data never leaves your device. It can chat, run tools like this one, and work with images and video. "
                        a href="https://gizza.ai" { "Visit gizza.ai →" }
                    }
                }
                script { (PreEscaped(format!("window.GIZZA_TOOL = {client_cfg};"))) }
                script type="module" src="./tool.js" {}
            }
        }
    };
    markup.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::ToolMeta;

    fn sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
subdomain     = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate expressions instantly."
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#,
        )
        .unwrap()
    }

    #[test]
    fn includes_seo_head_and_widget() {
        let html = render_page(&sample(), "<h2>About</h2>");
        assert!(html.contains("<title>Free Online Calculator — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://calculator.gizza.ai/">"#));
        assert!(html.contains("application/ld+json"));
        assert!(html.contains(r#"id="in-expr""#));
        assert!(html.contains(r#"id="tool-output""#));
        assert!(html.contains("window.GIZZA_TOOL"));
        assert!(html.contains("Powered by gizza.ai"));
        assert!(html.contains("<h2>About</h2>"));
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cd $ROOT/tools/generator && cargo test template::`
Expected: PASS, 1 test.

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add tools/generator/src/template.rs
git commit -m "feat(generator): option-C page template with SEO head + JSON-LD"
```

---

## Task 8: Generator — main driver (scan blocks, render, write pkg)

**Files:**
- Modify: `tools/generator/src/main.rs`

- [ ] **Step 1: Implement `run()`** — replace `tools/generator/src/main.rs`

```rust
//! gizza-tool-pages — renders standalone static pages for every tool that has
//! a `blocks/<tool>/page/meta.toml`, into `pkg/tools/<tool>/`.
//!
//! Usage: `gizza-tool-pages <repo_root>` (defaults to current dir).
//! Assumes each tool's wasm-pack output already exists at
//! `blocks/<tool>/web/pkg/`.

mod meta;
mod seo;
mod template;

use std::fs;
use std::path::{Path, PathBuf};

use meta::ToolMeta;

fn main() {
    if let Err(e) = run() {
        eprintln!("gizza-tool-pages: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let root = PathBuf::from(root);
    let blocks = root.join("blocks");
    let pkg_tools = root.join("pkg").join("tools");

    let metas = collect_tool_metas(&blocks)?;
    if metas.is_empty() {
        eprintln!("no tool pages found (no blocks/*/page/meta.toml)");
    }

    for (tool_dir, m) in &metas {
        let out = pkg_tools.join(&m.subdomain);
        fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;

        // 1. Render index.html.
        let content_md = fs::read_to_string(tool_dir.join("page/content.md"))
            .map_err(|e| format!("read content.md for {}: {e}", m.subdomain))?;
        let content_html = render_markdown(&content_md);
        let html = template::render_page(m, &content_html);
        fs::write(out.join("index.html"), html)
            .map_err(|e| format!("write index.html: {e}"))?;

        // 2. Copy the tool's wasm-pack output (js glue + bg.wasm).
        let web_pkg = tool_dir.join("web/pkg");
        copy_file(&web_pkg.join(format!("{}.js", m.wasm)), &out.join(format!("{}.js", m.wasm)))?;
        copy_file(
            &web_pkg.join(format!("{}_bg.wasm", m.wasm)),
            &out.join(format!("{}_bg.wasm", m.wasm)),
        )?;

        // 3. Copy shared page runtime.
        copy_file(&root.join("site/tool.js"), &out.join("tool.js"))?;
        copy_file(&root.join("site/tool.css"), &out.join("tool.css"))?;
        eprintln!("rendered tools/{}/", m.subdomain);
    }

    // 4. SEO site-wide files at pkg root.
    let subdomains: Vec<String> = metas.iter().map(|(_, m)| m.subdomain.clone()).collect();
    let pkg = root.join("pkg");
    fs::write(pkg.join("sitemap.xml"), seo::sitemap(&subdomains))
        .map_err(|e| format!("write sitemap.xml: {e}"))?;
    fs::write(pkg.join("robots.txt"), seo::robots())
        .map_err(|e| format!("write robots.txt: {e}"))?;

    Ok(())
}

/// Find every `blocks/<tool>/page/meta.toml`, parse it, sorted by subdomain.
fn collect_tool_metas(blocks: &Path) -> Result<Vec<(PathBuf, ToolMeta)>, String> {
    let mut out = Vec::new();
    if !blocks.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(blocks).map_err(|e| format!("read blocks/: {e}"))? {
        let entry = entry.map_err(|e| format!("blocks entry: {e}"))?;
        let meta_path = entry.path().join("page/meta.toml");
        if !meta_path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&meta_path)
            .map_err(|e| format!("read {}: {e}", meta_path.display()))?;
        let m = ToolMeta::from_toml(&text)?;
        out.push((entry.path(), m));
    }
    out.sort_by(|a, b| a.1.subdomain.cmp(&b.1.subdomain));
    Ok(out)
}

/// Render markdown to an HTML fragment.
fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Parser};
    let parser = Parser::new(md);
    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    buf
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}
```

- [ ] **Step 2: Build to verify it compiles** (seo fns come in Task 9; add temporary stubs first)

Add temporary stubs to `tools/generator/src/seo.rs` so this compiles now:

```rust
//! Sitemap / robots / JSON-LD helpers.

/// Build a sitemap listing the apex site and every tool subdomain.
pub fn sitemap(subdomains: &[String]) -> String {
    let mut urls = String::from("  <url><loc>https://gizza.ai/</loc></url>\n");
    for s in subdomains {
        urls.push_str(&format!("  <url><loc>https://{s}.gizza.ai/</loc></url>\n"));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{urls}</urlset>\n"
    )
}

/// robots.txt allowing all and pointing at the sitemap.
pub fn robots() -> String {
    "User-agent: *\nAllow: /\nSitemap: https://gizza.ai/sitemap.xml\n".to_string()
}
```

Run: `cd $ROOT/tools/generator && cargo build`
Expected: builds clean.

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add tools/generator/src/main.rs tools/generator/src/seo.rs
git commit -m "feat(generator): scan blocks, render pages, copy assets, emit sitemap/robots"
```

---

## Task 9: Generator — SEO helpers with tests

**Files:**
- Modify: `tools/generator/src/seo.rs`

- [ ] **Step 1: Add tests** — append to `tools/generator/src/seo.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_lists_apex_and_subdomains() {
        let xml = sitemap(&["calculator".into(), "clock".into()]);
        assert!(xml.contains("<loc>https://gizza.ai/</loc>"));
        assert!(xml.contains("<loc>https://calculator.gizza.ai/</loc>"));
        assert!(xml.contains("<loc>https://clock.gizza.ai/</loc>"));
        assert!(xml.starts_with("<?xml"));
    }

    #[test]
    fn robots_points_at_sitemap() {
        let txt = robots();
        assert!(txt.contains("Sitemap: https://gizza.ai/sitemap.xml"));
        assert!(txt.contains("User-agent: *"));
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cd $ROOT/tools/generator && cargo test seo::`
Expected: PASS, 2 tests.

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add tools/generator/src/seo.rs
git commit -m "test(generator): cover sitemap + robots"
```

---

## Task 10: Shared client runtime — tool.css

**Files:**
- Create: `site/tool.css`

- [ ] **Step 1: Write the stylesheet** — `site/tool.css`

```css
/* Standalone tool page — option C layout. Pairs with site-kit design-system.css. */
:root { --tool-accent: #4f46e5; --tool-ink: #0f172a; --tool-muted: #6b7280; }
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; color: var(--tool-ink); background: #fff; }

.tool-nav { display: flex; justify-content: space-between; align-items: center;
  padding: 12px 20px; border-bottom: 1px solid #e5e7eb; max-width: 880px; margin: 0 auto; }
.tool-brand { display: flex; align-items: center; gap: 8px; font-weight: 700;
  text-decoration: none; color: var(--tool-ink); }
.tool-brand img { width: 30px; height: 30px; object-fit: contain; }
.tool-chat-link { color: var(--tool-accent); font-weight: 600; text-decoration: none; font-size: 14px; }

.tool-main { max-width: 880px; margin: 0 auto; padding: 24px 20px 0; }
.tool-hero { text-align: center; padding: 12px 0 8px; }
.tool-hero h1 { font-size: clamp(24px, 5vw, 34px); font-weight: 800; margin: 4px 0; }
.tool-hero-sub { color: var(--tool-muted); max-width: 460px; margin: 0 auto; }

.tool-widget { border: 2px solid #d1d5db; border-radius: 12px; padding: 18px;
  background: #fafafa; max-width: 380px; margin: 18px auto 0; text-align: left; }
.tool-field-label, .tool-output-label { font-size: 11px; text-transform: uppercase;
  letter-spacing: .04em; color: #94a3b8; display: block; }
.tool-input { width: 100%; background: #fff; border: 1px solid #cbd5e1; border-radius: 8px;
  padding: 10px; margin: 6px 0 12px; font-size: 16px; }
.tool-input:focus { outline: 2px solid var(--tool-accent); border-color: var(--tool-accent); }
.tool-output { display: block; background: #eef2ff; border-radius: 8px; padding: 12px;
  font-weight: 700; font-size: 20px; min-height: 1.4em; word-break: break-word; }
.tool-output.error { background: #fef2f2; color: #b91c1c; font-size: 14px; font-weight: 600; }

.tool-content { max-width: 720px; margin: 28px auto 0; line-height: 1.6; color: #374151; }
.tool-content h2 { font-size: 20px; margin-top: 24px; }
.tool-content h3 { font-size: 16px; margin-top: 18px; }
.tool-content code { background: #f1f5f9; padding: 1px 5px; border-radius: 4px; font-size: .9em; }

.tool-footer { background: var(--tool-ink); color: #94a3b8; margin-top: 36px;
  padding: 22px 20px; }
.tool-footer > * { max-width: 720px; margin-left: auto; margin-right: auto; }
.tool-footer-brand { display: flex; align-items: center; gap: 8px; font-weight: 700; color: #fff; }
.tool-footer-brand img { width: 26px; height: 26px; object-fit: contain; }
.tool-footer p { font-size: 13px; line-height: 1.6; margin: 10px auto 0; }
.tool-footer a { color: #a5b4fc; font-weight: 600; text-decoration: none; }
```

- [ ] **Step 2: Commit**

```bash
cd $ROOT
git add site/tool.css
git commit -m "feat(tool-page): shared option-C stylesheet"
```

---

## Task 11: Shared client runtime — tool.js

**Files:**
- Create: `site/tool.js`

- [ ] **Step 1: Write the driver** — `site/tool.js`

```javascript
// Generic standalone-tool driver. Reads window.GIZZA_TOOL (baked by the page
// generator), loads the tool's wasm-bindgen module, wires inputs to the
// exported function, and renders the result. Shared by every tool subdomain.

const cfg = window.GIZZA_TOOL;
const out = document.getElementById(cfg.output.elementId);

function showResult(value) {
  out.classList.remove("error");
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
}

function showError(message) {
  out.classList.add("error");
  out.textContent = message;
}

function formatNumber(v) {
  // Trim float noise without forcing decimals on integers.
  return Number.isFinite(v) ? String(Math.round(v * 1e12) / 1e12) : String(v);
}

// Collect call args in declared order. "field" → input value; "clock" → now (s).
function gatherArgs() {
  return cfg.inputs.map((inp) => {
    if (inp.source === "clock") return Math.floor(Date.now() / 1000);
    const el = document.getElementById(inp.elementId);
    return el ? el.value : "";
  });
}

async function main() {
  let mod;
  try {
    mod = await import(cfg.module);
    await mod.default(); // wasm-pack --target web init
  } catch (e) {
    showError("Failed to load tool.");
    return;
  }
  const fn = mod[cfg.export];

  function compute() {
    try {
      const result = fn(...gatherArgs());
      showResult(result);
    } catch (e) {
      const msg = typeof e === "string" ? e : e && e.message ? e.message : "error";
      // Don't shout at the user for an empty field.
      const hasField = cfg.inputs.some((i) => i.source === "field");
      const empty = hasField && gatherArgs().every((a) => a === "" || a == null);
      if (empty) {
        out.classList.remove("error");
        out.textContent = "";
      } else {
        showError(msg);
      }
    }
  }

  // Wire field inputs to live recompute.
  for (const inp of cfg.inputs) {
    if (inp.source === "field") {
      const el = document.getElementById(inp.elementId);
      if (el) el.addEventListener("input", compute);
    }
  }

  if (cfg.live) {
    compute();
    setInterval(compute, cfg.intervalMs || 1000);
  } else {
    compute(); // initial (e.g. prefilled / empty state)
  }
}

main();
```

- [ ] **Step 2: Commit**

```bash
cd $ROOT
git add site/tool.js
git commit -m "feat(tool-page): generic client driver (window.GIZZA_TOOL)"
```

---

## Task 12: Clock — core, block, web, page

**Files:**
- Create: `blocks/clock/core/Cargo.toml`, `blocks/clock/core/src/lib.rs`
- Modify: `blocks/clock/Cargo.toml`, `blocks/clock/src/lib.rs`
- Create: `blocks/clock/web/Cargo.toml`, `blocks/clock/web/src/lib.rs`
- Create: `blocks/clock/page/meta.toml`, `blocks/clock/page/content.md`

- [ ] **Step 1: Write the core with its test** — `blocks/clock/core/src/lib.rs`

```rust
//! gizza-ai/clock core — UTC ISO-8601 formatting shared by the chat skill
//! block and the standalone web page. Pure; no IO, no wafer/wasm-bindgen.

/// Format a Unix timestamp (seconds) as UTC ISO-8601, e.g. "2026-05-30T12:00:00Z".
/// Uses Howard Hinnant's civil-from-days algorithm.
pub fn format_iso8601(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, hh, mm, ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_unix_zero() {
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_timestamp() {
        // 1_700_000_000 = 2023-11-14T22:13:20Z
        assert_eq!(format_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
```

- [ ] **Step 2: Core manifest** — `blocks/clock/core/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["."]

[package]
name = "gizza-ai-clock-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[dependencies]
```

- [ ] **Step 3: Run core test**

Run: `cd $ROOT/blocks/clock/core && cargo test`
Expected: PASS, 2 tests.

- [ ] **Step 4: Block delegates to core** — add dep to `blocks/clock/Cargo.toml` `[dependencies]`:

```toml
gizza-ai-clock-core = { path = "core" }
```

Then replace `blocks/clock/src/lib.rs` with:

```rust
//! gizza-ai/clock — returns the current time.
//!
//! Thin chat-skill wrapper around `gizza-ai-clock-core`. Takes no args, returns
//! `{ "iso": "...", "unix": <secs> }`. Uses the host clock via
//! `wafer_sdk::now_unix_millis()` so it works identically native + wasm.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
struct Clock;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/clock",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clock skill",
    skill(
        description = "Return the current UTC date and time. No arguments needed.",
        parameters = r#"{
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }"#
    ),
)]
impl Clock {
    fn handle(_msg: Message, _body: Vec<u8>) -> GuestResult {
        let millis = now_unix_millis();
        let secs = millis / 1000;
        let iso = gizza_ai_clock_core::format_iso8601(secs);
        let body = serde_json::json!({ "iso": iso, "unix": secs });
        GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
    }
}
```

- [ ] **Step 5: Verify block builds (native)**

Run: `cd $ROOT/blocks/clock && cargo build`
Expected: builds clean.

- [ ] **Step 6: Web wrapper** — `blocks/clock/web/src/lib.rs`

```rust
//! Browser-facing wasm-bindgen wrapper around `gizza-ai-clock-core`.

use wasm_bindgen::prelude::*;

/// Format a Unix timestamp (seconds, supplied by JS) as UTC ISO-8601.
#[wasm_bindgen]
pub fn format_time(unix_secs: i64) -> String {
    gizza_ai_clock_core::format_iso8601(unix_secs)
}
```

`blocks/clock/web/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["."]

[package]
name = "gizza-ai-clock-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
gizza-ai-clock-core = { path = "../core" }
```

- [ ] **Step 7: Build clock web wasm**

Run: `cd $ROOT/blocks/clock/web && wasm-pack build --target web --release --out-dir pkg`
Expected: produces `pkg/gizza_ai_clock_web.js` + `pkg/gizza_ai_clock_web_bg.wasm`.

- [ ] **Step 8: Page metadata** — `blocks/clock/page/meta.toml`

```toml
subdomain     = "clock"
title         = "Current UTC Time — gizza.ai"
description   = "See the current UTC date and time, updating live in your browser. No sign-up, runs offline."
h1            = "Current UTC Time"
hero_subtitle = "The current Coordinated Universal Time, ticking live in your browser."
wasm          = "gizza_ai_clock_web"
export        = "format_time"
live          = true
interval_ms   = 1000
output_label  = "Current time (UTC)"
format        = "text"

[[input]]
name   = "unix"
source = "clock"
```

- [ ] **Step 9: Page content** — `blocks/clock/page/content.md`

```markdown
## About this clock

This page shows the current UTC (Coordinated Universal Time) date and time,
updating every second. The time is read from your own device's clock — nothing
is sent to a server, and it works offline.

### What is UTC?

UTC is the global time standard that clocks and time zones are based on. Unlike
local time, it does not change with daylight saving.

### FAQ

**Why UTC?** It is unambiguous worldwide — useful for logs, scheduling across
time zones, and coordination.

**Is it accurate?** It is as accurate as your device's clock.
```

- [ ] **Step 10: Commit**

```bash
cd $ROOT
git add blocks/clock
git commit -m "feat(clock): core/web/page — second tool on the shared system"
```

---

## Task 13: Build wiring — justfile recipe

**Files:**
- Modify: `justfile`

- [ ] **Step 1: Replace `justfile`**

```makefile
# gizza-ai — build & serve
#
# `just` with no args lists recipes.

default:
    @just --list

# Build per-tool web wasm + render standalone tool pages into pkg/tools/.
# Run AFTER `solobase build` (which creates pkg/).
build-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    for dir in blocks/*/page; do
        tool="$(basename "$(dirname "$dir")")"
        echo "building web wasm for $tool"
        wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
    done
    cargo run --manifest-path tools/generator/Cargo.toml -- .

serve port="8001":
    solobase build
    just build-tools
    cd pkg && python3 -m http.server {{port}}

test:
    cargo test

test-generator:
    cargo test --manifest-path tools/generator/Cargo.toml

test-routing:
    node --test functions/routing.test.mjs

test-e2e:
    npx playwright test
```

- [ ] **Step 2: Commit**

```bash
cd $ROOT
git add justfile
git commit -m "build: just build-tools recipe + test recipes"
```

---

## Task 14: Main app "Tools" interlink — build.rs scan

**Files:**
- Modify: `build.rs`

- [ ] **Step 1: Extend build.rs to emit a TOOLS table** — append before the final `fs::write(&dest, ...)` for skills, then add a second generated file. Add this block to `build.rs` `main()` after the skills `entries` are written (i.e. after the existing `fs::write(&dest, out)` call):

```rust
    // --- Tool pages: scan blocks/<tool>/page/meta.toml for the index "Tools"
    // section so the main app links out to each subdomain (single source of
    // truth = the same meta.toml the page generator reads).
    let mut tools: Vec<(String, String)> = Vec::new(); // (subdomain, h1)
    if blocks_dir.is_dir() {
        for entry in fs::read_dir(&blocks_dir).expect("read blocks/ for tools") {
            let entry = entry.expect("blocks/ entry");
            let meta_path = entry.path().join("page/meta.toml");
            if !meta_path.is_file() {
                continue;
            }
            let text = fs::read_to_string(&meta_path).expect("read meta.toml");
            // Minimal extraction without a toml dep: pull subdomain + h1 lines.
            let sub = toml_str_value(&text, "subdomain");
            let h1 = toml_str_value(&text, "h1");
            if let (Some(sub), Some(h1)) = (sub, h1) {
                tools.push((sub, h1));
            }
            println!("cargo:rerun-if-changed={}", meta_path.display());
        }
    }
    tools.sort_by(|a, b| a.0.cmp(&b.0));

    let mut tout = String::new();
    tout.push_str("// Generated by build.rs — do not edit.\n");
    tout.push_str("// (subdomain, title) for every tool with a page/meta.toml.\n\n");
    tout.push_str("pub const TOOLS: &[(&str, &str)] = &[\n");
    for (sub, h1) in &tools {
        let h1_escaped = h1.replace('\\', "\\\\").replace('"', "\\\"");
        tout.push_str(&format!("    (\"{sub}\", \"{h1_escaped}\"),\n"));
    }
    tout.push_str("];\n");
    let tdest = PathBuf::from(&out_dir).join("tools.rs");
    fs::write(&tdest, tout).expect("write tools.rs");
```

And add this helper function at the bottom of `build.rs`:

```rust
/// Extract a top-level `key = "value"` string from TOML text without pulling in
/// a toml parser. Returns the first match.
fn toml_str_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim();
                if rest.starts_with('"') && rest.len() >= 2 {
                    if let Some(end) = rest[1..].find('"') {
                        return Some(rest[1..=end].to_string());
                    }
                }
            }
        }
    }
    None
}
```

- [ ] **Step 2: Verify build.rs change compiles via a native build of the gizza-ai crate** (heavy — pulls git deps; allowed network)

Run: `cd $ROOT && cargo build 2>&1 | tail -20`
Expected: builds; `target/.../out/tools.rs` exists with calculator + clock entries.

> If the full crate build is too slow/blocked in your environment, at minimum confirm `build.rs` itself parses by running `rustc --edition 2021 --crate-type bin build.rs -o /dev/null` after temporarily stubbing the `solobase_pin`/serde_json uses — but prefer the real build.

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add build.rs
git commit -m "build: generate TOOLS table from page/meta.toml for index interlink"
```

---

## Task 15: Main app "Tools" interlink — render section in ui.rs

**Files:**
- Modify: `src/blocks/ui.rs`

- [ ] **Step 1: Locate the include + render site.** Open `src/blocks/ui.rs`, find where `render_chat()` builds the page body (where the existing brand/header maud markup is). Add this `include!` near the top of the file (after the module's `use` statements):

```rust
// Generated by build.rs: pub const TOOLS: &[(&str, &str)] = ...
include!(concat!(env!("OUT_DIR"), "/tools.rs"));
```

- [ ] **Step 2: Add a Tools section to the rendered markup.** Inside the `maud::html! { ... }` block that renders the chat page (in `render_chat` or equivalent), add — placed after the main chat container, before the closing body content — the following markup:

```rust
                @if !TOOLS.is_empty() {
                    section class="gizza-tools" aria-label="Standalone tools" {
                        h2 { "Tools" }
                        ul {
                            @for (sub, title) in TOOLS {
                                li {
                                    a href=(format!("https://{sub}.gizza.ai")) { (title) }
                                }
                            }
                        }
                    }
                }
```

> If `render_chat` returns `Markup`/`PreEscaped(String)` built differently, insert the same nodes in the corresponding spot. The key invariant: iterate `TOOLS` and emit one `<a href="https://{sub}.gizza.ai">` per tool.

- [ ] **Step 3: Add minimal styling** — append to `site/gizza.css`:

```css
/* Standalone tools interlink (SEO + discoverability). */
.gizza-tools { max-width: 720px; margin: 24px auto; padding: 0 16px; }
.gizza-tools h2 { font-size: 16px; margin: 0 0 8px; }
.gizza-tools ul { list-style: none; padding: 0; margin: 0; display: flex; flex-wrap: wrap; gap: 8px; }
.gizza-tools a { display: inline-block; padding: 6px 12px; border: 1px solid #d1d5db;
  border-radius: 999px; text-decoration: none; color: #4f46e5; font-size: 14px; }
.gizza-tools a:hover { background: #eef2ff; }
```

- [ ] **Step 4: Verify it compiles**

Run: `cd $ROOT && cargo build 2>&1 | tail -20`
Expected: builds clean.

- [ ] **Step 5: Commit**

```bash
cd $ROOT
git add src/blocks/ui.rs site/gizza.css
git commit -m "feat(ui): Tools interlink section on the index page"
```

---

## Task 16: Cloudflare routing — pure mapping + test

**Files:**
- Create: `functions/routing.mjs`
- Create: `functions/routing.test.mjs`

- [ ] **Step 1: Write the failing test** — `functions/routing.test.mjs`

```javascript
import { test } from "node:test";
import assert from "node:assert/strict";
import { resolve } from "./routing.mjs";

test("apex serves the app unchanged", () => {
  assert.deepEqual(resolve("gizza.ai", "/"), { type: "app", path: "/" });
  assert.deepEqual(resolve("www.gizza.ai", "/foo"), { type: "app", path: "/foo" });
});

test("tool subdomain rewrites to /tools/<sub>/...", () => {
  assert.deepEqual(resolve("calculator.gizza.ai", "/"), {
    type: "tool",
    path: "/tools/calculator/index.html",
  });
  assert.deepEqual(resolve("clock.gizza.ai", "/tool.css"), {
    type: "tool",
    path: "/tools/clock/tool.css",
  });
});

test("host with port is handled", () => {
  assert.deepEqual(resolve("calculator.gizza.ai:443", "/"), {
    type: "tool",
    path: "/tools/calculator/index.html",
  });
});

test("localhost and pages.dev serve the app", () => {
  assert.equal(resolve("localhost", "/").type, "app");
  assert.equal(resolve("gizza-ai.pages.dev", "/").type, "app");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd $ROOT && node --test functions/routing.test.mjs`
Expected: FAIL (cannot find module `./routing.mjs`).

- [ ] **Step 3: Implement** — `functions/routing.mjs`

```javascript
// Pure host→path mapping for gizza.ai. Unit-tested independently of Cloudflare.
// apex/www/localhost/pages.dev → main app; <sub>.gizza.ai → /tools/<sub>/...

const APEX = "gizza.ai";

/**
 * @param {string} host - request Host header (may include :port)
 * @param {string} pathname - request path (starts with "/")
 * @returns {{type:"app"|"tool"|"redirect", path:string, location?:string}}
 */
export function resolve(host, pathname) {
  const h = (host || "").split(":")[0].toLowerCase();

  // Non-production hosts: always the app.
  if (h === "localhost" || h === "127.0.0.1" || h.endsWith(".pages.dev")) {
    return { type: "app", path: pathname };
  }

  if (h === APEX || h === `www.${APEX}`) {
    return { type: "app", path: pathname };
  }

  if (h.endsWith(`.${APEX}`)) {
    const sub = h.slice(0, -1 * (`.${APEX}`).length);
    // Single-label subdomains only (calculator, clock).
    if (sub && !sub.includes(".")) {
      const tail = pathname === "/" ? "/index.html" : pathname;
      return { type: "tool", path: `/tools/${sub}${tail}` };
    }
  }

  // Unknown host → send to apex.
  return { type: "redirect", path: pathname, location: `https://${APEX}/` };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd $ROOT && node --test functions/routing.test.mjs`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
cd $ROOT
git add functions/routing.mjs functions/routing.test.mjs
git commit -m "feat(routing): pure host->path resolver with tests"
```

---

## Task 17: Cloudflare Pages middleware

**Files:**
- Create: `functions/_middleware.js`

- [ ] **Step 1: Write the middleware** — `functions/_middleware.js`

```javascript
// Cloudflare Pages Function: rewrite tool subdomains to /tools/<sub>/ and serve
// the corresponding static asset. apex/www serve the app unchanged.
import { resolve } from "./routing.mjs";

export async function onRequest(context) {
  const { request, next } = context;
  const url = new URL(request.url);
  const decision = resolve(url.hostname, url.pathname);

  if (decision.type === "redirect") {
    return Response.redirect(decision.location, 302);
  }

  if (decision.type === "tool") {
    const rewritten = new URL(request.url);
    rewritten.pathname = decision.path;
    // Serve the static asset at the rewritten path.
    return context.env.ASSETS.fetch(new Request(rewritten.toString(), request));
  }

  // app: continue to normal static asset serving.
  return next();
}
```

- [ ] **Step 2: Sanity-check syntax**

Run: `cd $ROOT && node --check functions/_middleware.js`
Expected: no output (valid syntax).

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add functions/_middleware.js
git commit -m "feat(routing): Cloudflare Pages middleware using resolver"
```

---

## Task 18: Playwright — standalone page e2e

**Files:**
- Create: `tests/tool_pages.spec.ts`

- [ ] **Step 1: Write the test** — `tests/tool_pages.spec.ts`

```typescript
import { test, expect } from "@playwright/test";

// Served from pkg/ by `just serve` (python http.server). Tool pages live under
// /tools/<sub>/ locally; the subdomain rewrite is exercised by routing.test.mjs.
const BASE = process.env.GIZZA_BASE_URL ?? "http://127.0.0.1:8001";

test("calculator page computes and has SEO tags", async ({ page }) => {
  await page.goto(`${BASE}/tools/calculator/`);

  // SEO essentials present in static HTML.
  await expect(page).toHaveTitle(/Free Online Calculator/);
  const desc = page.locator('meta[name="description"]');
  await expect(desc).toHaveAttribute("content", /browser/i);
  const ld = page.locator('script[type="application/ld+json"]');
  await expect(ld).toHaveCount(1);

  // Branding + footer.
  await expect(page.locator(".tool-brand")).toContainText("gizza.ai");
  await expect(page.locator(".tool-footer")).toContainText("Powered by gizza.ai");

  // Compute: type an expression, expect a result.
  await page.fill("#in-expr", "2 + 2 * 3");
  await expect(page.locator("#tool-output")).toHaveText("8", { timeout: 10_000 });
});

test("clock page shows a live UTC timestamp", async ({ page }) => {
  await page.goto(`${BASE}/tools/clock/`);
  await expect(page).toHaveTitle(/Current UTC Time/);
  await expect(page.locator("#tool-output")).toHaveText(
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/,
    { timeout: 10_000 }
  );
});
```

- [ ] **Step 2: Build everything and run the test**

Run:
```bash
cd $ROOT
solobase build
just build-tools
( cd pkg && python3 -m http.server 8001 & echo $! > /tmp/gizza_http.pid )
sleep 2
npx playwright test tests/tool_pages.spec.ts
kill "$(cat /tmp/gizza_http.pid)"
```
Expected: 2 passed. (First run installs the Playwright browser if needed: `npx playwright install chromium`.)

- [ ] **Step 3: Commit**

```bash
cd $ROOT
git add tests/tool_pages.spec.ts
git commit -m "test(e2e): standalone calculator + clock pages"
```

---

## Task 19: Deploy migration — Cloudflare Pages + retire GitHub Pages

**Files:**
- Modify: `.github/workflows/deploy.yml`
- Delete: `static/CNAME`
- Modify: `solobase.toml` (remove the CNAME overlay)

- [ ] **Step 1: Remove the CNAME overlay from `solobase.toml`.** Delete this block:

```toml
[[assets.overlay]]
from = "static/CNAME"
to = "CNAME"
```

- [ ] **Step 2: Delete the GitHub Pages CNAME file**

```bash
cd $ROOT
git rm static/CNAME
```

- [ ] **Step 3: Add tool.js / tool.css to the asset bypass list** so the dev server / solobase serves them — edit `solobase.toml` `extra_bypass_prefix` array to include `"/tool.js", "/tool.css"`. The line becomes (append the two entries):

```toml
extra_bypass_prefix = ["/gizza-app.js", "/gizza.css", "/render.js", "/pending.js", "/gis.png", "/gis_no_eyes.png", "/gis_a_job_no_eyes.png", "/eye.png", "/gis_video_idle.mp4", "/gis_video_typing_loop.mp4", "/gis_video_typing_finish.mp4", "/favicon.ico", "/favicon-32.png", "/apple-touch-icon.png", "/model-picker.js", "/model-picker.css", "/tool.js", "/tool.css"]
```

- [ ] **Step 4: Rewrite `.github/workflows/deploy.yml`** — replace the file with:

```yaml
name: Deploy to Cloudflare Pages

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout gizza-ai
        uses: actions/checkout@v4
        with:
          path: gizza-ai

      - name: Read solobase pin
        id: pin
        run: echo "solobase_sha=$(cat gizza-ai/solobase-pin.txt | tr -d '[:space:]')" >> "$GITHUB_OUTPUT"

      - name: Checkout solobase
        uses: actions/checkout@v4
        with:
          repository: suppers-ai/solobase
          ref: ${{ steps.pin.outputs.solobase_sha }}
          path: solobase

      - name: Checkout wafer-run
        uses: actions/checkout@v4
        with:
          repository: wafer-run/wafer-run
          ref: main
          path: wafer-run

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown,wasm32-wasip1

      - name: Install wasm-pack
        uses: jetli/wasm-pack-action@v0.4.0

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: |
            gizza-ai
            solobase
            wafer-run

      - name: Build solobase-web wasm
        working-directory: solobase
        run: wasm-pack build crates/solobase-web --target web --release --out-dir pkg

      - name: Build solobase CLI
        run: cargo install --path solobase/crates/solobase --locked

      - name: Build wafer CLI
        run: cargo install --path wafer-run/crates/wafer-cli --locked

      - name: Build site
        working-directory: gizza-ai
        run: solobase build

      - name: Build tool pages
        working-directory: gizza-ai
        run: |
          for dir in blocks/*/page; do
            tool="$(basename "$(dirname "$dir")")"
            wasm-pack build "blocks/$tool/web" --target web --release --out-dir pkg
          done
          cargo run --manifest-path tools/generator/Cargo.toml -- .

      - name: Deploy to Cloudflare Pages
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          command: pages deploy gizza-ai/pkg --project-name=gizza-ai --branch=main
```

- [ ] **Step 5: Commit**

```bash
cd $ROOT
git add .github/workflows/deploy.yml solobase.toml
git commit -m "ci: deploy to Cloudflare Pages; retire GitHub Pages + CNAME"
```

---

## Task 20: Operator runbook (manual Cloudflare steps)

**Files:**
- Create: `docs/superpowers/handoffs/2026-05-30-cloudflare-tool-subdomains-runbook.md`

- [ ] **Step 1: Write the runbook** — `docs/superpowers/handoffs/2026-05-30-cloudflare-tool-subdomains-runbook.md`

```markdown
# Cloudflare tool-subdomains — operator runbook

Manual, account-level steps (done by a human; the CI changes are already in the repo).

## One-time setup

1. **Create the Pages project** named `gizza-ai` (Cloudflare dashboard → Workers & Pages
   → Create → Pages → Direct Upload, or let the first `wrangler pages deploy` create it).
2. **Custom domains** (Pages project → Custom domains):
   - Add `gizza.ai`
   - Add `www.gizza.ai`
   - Add `*.gizza.ai` (wildcard) — this is what makes `calculator.gizza.ai` etc. resolve.
3. **DNS** (gizza.ai zone): ensure proxied CNAMEs exist for `@`, `www`, and `*` pointing at
   the Pages project (`gizza-ai.pages.dev`). Cloudflare usually adds these when you attach
   custom domains.
4. **Repo secrets** (GitHub → repo → Settings → Secrets → Actions):
   - `CLOUDFLARE_API_TOKEN` — token with "Cloudflare Pages: Edit" permission.
   - `CLOUDFLARE_ACCOUNT_ID` — your account id.
5. **Retire GitHub Pages**: repo Settings → Pages → set Source to "None" (the old
   `Deploy to GitHub Pages` workflow has been replaced).

## Verify after first deploy

- `https://gizza.ai/` → chat app loads, "Tools" section links to subdomains.
- `https://calculator.gizza.ai/` → calculator page; typing `2+2*3` shows `8`.
- `https://clock.gizza.ai/` → live UTC timestamp.
- `https://gizza.ai/sitemap.xml` lists apex + every tool subdomain.
- View source on a tool page: `<title>`, meta description, and JSON-LD present.

## Adding a future tool

Create `blocks/<tool>/{core,web,page}/` following calculator's shape (a `core` rlib, a
`web` wasm-bindgen cdylib, and `page/meta.toml` + `page/content.md`). The build, sitemap,
index interlink, and routing all pick it up automatically — no per-tool wiring.
```

- [ ] **Step 2: Commit**

```bash
cd $ROOT
git add docs/superpowers/handoffs/2026-05-30-cloudflare-tool-subdomains-runbook.md
git commit -m "docs: Cloudflare tool-subdomains operator runbook"
```

---

## Final verification

- [ ] **All Rust unit tests pass**

Run:
```bash
cd $ROOT
cargo test --manifest-path blocks/calculator/core/Cargo.toml
cargo test --manifest-path blocks/clock/core/Cargo.toml
cargo test --manifest-path tools/generator/Cargo.toml
```
Expected: all green.

- [ ] **Routing tests pass:** `cd $ROOT && node --test functions/routing.test.mjs` → 4 passed.

- [ ] **Full build + page e2e pass** (Task 18 command block) → 2 passed.

- [ ] **Chat regression intact:** `cd $ROOT && cargo test` (root) → existing `dispatch_skills` behavior unchanged.

---

## Self-Review notes (author)

**Spec coverage:**
- Single-source logic (core + block + web): Tasks 1–3, 12 ✓
- Template option C + meta/content: Tasks 4, 7, 12 ✓
- Generator + build integration: Tasks 5–9, 13 ✓
- Shared tool.js/tool.css: Tasks 10–11 ✓
- Cloudflare wildcard routing: Tasks 16–17 ✓
- SEO (head, JSON-LD, sitemap, robots, interlink): Tasks 7, 9, 14–15 ✓
- Cloudflare deploy migration + retire GH Pages: Task 19 ✓
- Testing (core, web via e2e, routing, page e2e, chat regression): Tasks 1,9,16,18 + Final ✓
- Operator/manual steps: Task 20 ✓

**Note on web-wrapper unit tests:** the spec mentioned `wasm-pack test --headless`; this plan
covers the web wrappers' behavior through the Playwright page e2e (Task 18) plus the core
crate unit tests (which hold all the real logic). This avoids a headless-browser toolchain
dependency in CI for a one-line pass-through wrapper. If desired later, add
`wasm-pack test --headless --chrome blocks/<tool>/web`.
```
