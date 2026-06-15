# Per-tool standalone subdomain pages — design

**Date:** 2026-05-30
**Status:** Draft for review
**Repo:** `gizza-ai`

## Goal

Give every gizza-ai tool ("skill") a standalone, static, SEO-friendly page on its own
subdomain — e.g. the calculator tool is reachable both inside the chat (`/calculator`)
**and** at `calculator.gizza.ai` as a simple page that does one thing well. The page
shares its compute logic with the chat skill (one source of truth, no drift) and follows
a single branded template. The objective is more Google impressions: one focused,
crawlable landing page per tool.

## Decisions (locked during brainstorming)

1. **Hosting:** migrate all of gizza.ai (main chat app + tool subdomains) to a single
   **Cloudflare Pages** project. Cloudflare supports wildcard `*.gizza.ai` custom domains;
   GitHub Pages does not. The existing GitHub Pages deploy is retired.
2. **Scope:** build the full reusable system now; prove it on the two **pure-compute**
   tools — **calculator** and **clock**. Heavy media tools (ffmpeg/image/video) are out of
   scope for this iteration but the system is designed so any tool plugs in later by adding
   a folder + metadata.
3. **Logic sharing:** a shared **pure Rust core** per tool, compiled two ways — into the
   chat skill block (unchanged behaviour) and into a tiny standalone `wasm-bindgen` module
   for the page. The page carries **no** wafer runtime, no wasmi interpreter, no WebLLM,
   and no chat UI — just a few KB of wasm.
4. **Template:** "top nav + hero tool + below-the-fold SEO content + dark footer". The
   footer contains "⚡ Powered by gizza.ai", an about-gizza.ai blurb, and a link to the main
   site.

## Non-goals

- Standalone pages for the heavy media tools (ffmpeg, image-*, video-*, image-fetch,
  web-fetch). These need the ffmpeg runtime / file-upload UIs / CORS proxying and are a
  separate effort.
- Any change to chat behaviour. The skill blocks behave exactly as today.
- Server-side rendering or any runtime backend for the tool pages — they are fully static.

## Architecture

A new **tool-pages subsystem**. For each in-scope tool:

```
blocks/<tool>/
  core/          (NEW)  pure logic crate — no wafer, no wasm-bindgen
  src/lib.rs     (EDIT) chat skill block; handle() calls core
  web/           (NEW)  wasm-bindgen cdylib wrapping core for the page
  page/          (NEW)
    meta.toml           page metadata (subdomain, title, H1, inputs, output label)
    content.md          long-form SEO copy (how-to, examples, FAQ)
```

Build output:

```
pkg/
  index.html ...        main chat app (as today)
  tools/<tool>/
    index.html          rendered from the shared template + meta.toml + content.md
    <tool>_web.js       wasm-bindgen JS glue
    <tool>_web_bg.wasm  tiny per-tool wasm
  tool.js, tool.css     shared page runtime + styles
  sitemap.xml, robots.txt
functions/
  _middleware.js (NEW)  Cloudflare Pages host→path rewrite
```

### Component 1 — shared core crate (`blocks/<tool>/core/`)

- New crate, e.g. `gizza-ai-calculator-core`, `crate-type = ["rlib"]`, **zero** dependency
  on `wafer-sdk`/`wafer-block`/`wasm-bindgen`. Only the logic deps (`meval` for calculator;
  `chrono` for clock).
- Exposes the pure function(s). Calculator:
  `pub fn evaluate(expr: &str) -> Result<f64, String>` — lifted verbatim from today's
  `evaluate_expr` in `blocks/calculator/src/lib.rs`, along with its unit tests.
- Clock: `pub fn now_utc_rfc3339() -> String` (and any formatting the chat skill already
  produces), so the page and chat agree exactly.

This is the single source of truth. Both consumers below compile from it.

### Component 2 — chat skill block (`blocks/<tool>/src/lib.rs`, edited)

- Depends on the new `core` crate.
- `handle()` keeps the same JSON-in/JSON-out contract and the same `#[wafer_block]`
  `skill(...)` declaration; it just delegates the computation to `core::evaluate`. No
  behaviour change, verified by the existing dispatch tests.
- The block's `manifest.json` and `target/block.wasm` continue to feed `build.rs`'s
  `SKILLS` table unchanged.

### Component 3 — page wasm wrapper (`blocks/<tool>/web/`)

- New crate, `crate-type = ["cdylib"]`, depends on `core` + `wasm-bindgen`.
- Exposes the logic directly to JS, e.g.:
  ```rust
  #[wasm_bindgen]
  pub fn evaluate(expr: &str) -> Result<f64, JsValue> {
      core::evaluate(expr).map_err(|e| JsValue::from_str(&e))
  }
  ```
- Built with `wasm-pack build blocks/<tool>/web --target web --release`, producing a
  few-KB `*_bg.wasm` + JS glue. No wafer ABI, runs as native browser wasm.

### Component 4 — page template + generator

- The template (approved layout) is defined **once** as a Rust `maud` function in a small
  build helper (an `xtask`-style binary or a `just` recipe + helper crate). It renders:
  top nav (mascot logo + "Open AI chat" link) → hero (H1 + short description + the tool
  widget) → "About this <tool>" section (from `content.md`, rendered to HTML) → footer
  ("Powered by gizza.ai" + about-blurb + link to `https://gizza.ai`).
- It also injects per-page SEO `<head>`: `<title>`, meta description, canonical, Open
  Graph/Twitter tags, and JSON-LD `WebApplication` structured data — all from `meta.toml`.
- Brand consistency: the template links the same `site-kit` design-system CSS the main app
  uses, plus a small `tool.css`.

`meta.toml` shape (example, calculator):

```toml
subdomain   = "calculator"
title       = "Free Online Calculator — gizza.ai"
h1          = "Free Online Calculator"
description = "Evaluate any arithmetic expression instantly in your browser. No sign-up, runs offline."
wasm        = "calculator_web"          # basename of the wasm-pack output

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
type        = "text"

output_label = "Result"
```

### Component 5 — shared page runtime (`tool.js`, `tool.css`)

- `tool.js`: imports the tool's wasm-bindgen module named in `meta.toml`, reads the input
  field(s), calls the exported function on input (debounced/on submit), and renders the
  result or error into the output area. Generic — driven entirely by the field config the
  generator bakes into the page (e.g. `data-` attributes).
- `tool.css`: the option-C styling shared across all tool pages.

### Component 6 — Cloudflare wildcard routing (`functions/_middleware.js`)

- A Cloudflare Pages Function middleware that inspects the `Host` header:
  - `<sub>.gizza.ai` → internally serve `/tools/<sub>/...`.
  - apex `gizza.ai` / `www.gizza.ai` → serve the main app at root.
  - Unknown subdomain → redirect to apex (or 404).
- The subdomain→path mapping is a pure function, unit-tested independently of Cloudflare.

### Build & deploy

- New `just` target (e.g. `just build-tools`) and/or extension of the existing build that,
  after `solobase build` produces `pkg/`:
  1. For each `blocks/<tool>/page/` present: `wasm-pack build blocks/<tool>/web`.
  2. Run the generator → `pkg/tools/<tool>/index.html` + copy wasm/JS glue.
  3. Copy shared `tool.js`/`tool.css`/brand assets into `pkg/`.
  4. Generate `pkg/sitemap.xml` (apex + every tool subdomain) and `pkg/robots.txt`.
- CI: replace the GitHub Pages steps in `.github/workflows/deploy.yml` with
  `wrangler pages deploy pkg --project-name gizza-ai` (Cloudflare API token + account id as
  repo secrets). The solobase-web / solobase CLI / wafer CLI build steps stay.
- DNS/Cloudflare: add `gizza.ai` and wildcard `*.gizza.ai` as custom domains on the Pages
  project; wildcard CNAME in DNS. Remove `static/CNAME` (GitHub Pages artifact) and its
  `solobase.toml` overlay.

### SEO

- All page content (H1, description, long-form `content.md`) is rendered into static HTML
  at build time → crawlers see it with zero JS execution.
- Per page: unique title + meta description, canonical URL, OG/Twitter cards, JSON-LD
  `WebApplication`.
- `sitemap.xml` enumerates every tool subdomain; `robots.txt` references it.
- **Interlinking:** the main gizza.ai gains a "Tools" section/links pointing to each tool
  subdomain; every tool page footer links back to gizza.ai. This cross-linking plus
  per-subdomain focused content is the core of the impressions strategy.

## Testing

- **Core crate:** unit tests for the logic (calculator's existing tests move here and keep
  passing; add clock formatting tests). Covers both chat and page in one place.
- **Web wrapper:** `wasm-pack test --headless --firefox blocks/<tool>/web` exercising the
  exported function (happy path + error path).
- **Chat regression:** existing `tests/dispatch_skills.rs` continues to pass unchanged.
- **Page e2e (Playwright, existing `tests/`):** load `pkg/tools/calculator/index.html`,
  type an expression, assert the result; assert presence of `<title>`, meta description,
  JSON-LD, the mascot logo, and the footer. Snapshots route to `.playwright-mcp/` per repo
  convention.
- **Routing:** unit test the `_middleware` subdomain→path function for apex, known
  subdomain, and unknown subdomain.

## Rollout

1. Land the calculator slice end-to-end (core → block → web → page → routing) behind the
   new build target, deployed to Cloudflare Pages, and verify `calculator.gizza.ai` live.
2. Add clock as the second tool (proves the system is genuinely reusable: only a `core`,
   a `web`, and a `page/` folder).
3. Cut over apex `gizza.ai` DNS to Cloudflare Pages; retire the GitHub Pages workflow.
4. Future tools (including media tools, when their standalone UX is designed) follow the
   same folder pattern.

## Considered and rejected

- **Load the full skill block on the page** (boot the wafer/wasmi runtime in the browser
  and call the existing block): reuses the exact ABI but ships a hundreds-of-KB-to-MB host
  runtime to run a one-line `meval::eval_str` through an interpreter. Rejected on page
  weight and SEO/perf grounds.
- **Reimplement logic in JS:** lightest page but duplicates logic and risks drift.
  Rejected — violates the single-source goal.
- **Feature-gate the existing block crate** to also emit a wasm-bindgen entry instead of a
  separate `core` crate: fewer crates, but risks wafer-ABI symbols leaking into the page
  bundle. Rejected for the cleaner pure-core boundary.
- **One Cloudflare Pages project per subdomain:** avoids middleware but means N projects to
  operate. Rejected for operational sprawl.
- **Path-based URLs (`gizza.ai/calculator`) instead of subdomains:** simplest, but weaker
  SEO separation than true subdomains, which is the stated goal.
