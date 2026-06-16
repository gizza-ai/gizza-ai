# Design: tool pages — subdomain → `/tools/<slug>` path cutover

**Date:** 2026-06-16
**Repo:** gizza-ai (no solobase change required)
**Supersedes:** the subdomain-routing portion of `2026-05-30-tool-subdomain-pages-design.md`. The standalone-page generation and per-tool bundle architecture from that design are unchanged; only the *hostname* presentation changes.

## Motivation

Per-tool **subdomains** (`calculator.gizza.ai`) don't scale on Cloudflare Pages: Pages custom domains reject wildcards (verified — the "Add a custom domain" UI rejects `*.gizza.ai`), so reaching thousands of tools would mean adding each subdomain by hand (and Pages caps custom domains per project at ~100). The only wildcard-capable route is a Workers route, which adds per-request Worker cost and a router to maintain.

Serving the tools at **paths on the apex** (`gizza.ai/tools/<slug>`) removes that entire problem: pure static assets, free and unmetered, no per-tool DNS / domain / Worker, scales without limit, and pools SEO ranking authority into one domain instead of fragmenting it across subdomains.

The standalone, lightweight per-tool bundles (their own ~21–130K wasm, no full runtime) are **already** built and served at `/tools/<slug>/` — that property comes from the separate per-tool build, not from the hostname. This cutover just changes the URL the world sees and removes the subdomain plumbing.

## Goals

- Tool pages live at `https://gizza.ai/tools/<slug>/` and serve as lightweight static pages.
- The runtime Service Worker does **not** boot for tool pages (they stay static; no full-runtime load).
- Canonical/OpenGraph/sitemap URLs point at the path form.
- The dead subdomain routing is removed.
- No wildcard DNS, no Worker, no per-tool custom domain. Apex + `www` keep serving the app.

## Non-goals (YAGNI)

- No `/tools/` index/landing page and no new "Tools" nav link in the app (tools stay discoverable via `sitemap.xml` and their own back-links to `gizza.ai`).
- No redirects from the old `<slug>.gizza.ai` hostnames: those never resolved publicly (DNS was never wired for subdomains), so there are no inbound links or search-index entries to preserve, and a redirect would require re-introducing the very subdomain infrastructure we're removing.
- No change to how tool pages or their wasm are *built*.

## Background: why subdomains dodged the Service Worker, and why paths need a bypass

`sw.js` (generated from solobase's `crates/solobase-bundle/assets/sw.js.tmpl`) boots the wafer/solobase wasm **inside a root-scoped Service Worker** (`loader.js` registers `/sw.js`, scope `/`, with `clients.claim()`), and its `fetch` handler intercepts every same-origin request not on an explicit bypass allow-list, routing it through `handle_request` (the in-browser backend).

- **Subdomains** are a *different origin*, so the apex `gizza.ai` SW never controls `calculator.gizza.ai` — the tool page is naturally outside the runtime.
- **Same-origin paths** (`gizza.ai/tools/calculator/`) fall under the root SW's scope. Without a bypass, the SW would intercept the tool page and run it through the runtime (boot the heavy wasm / 404). **The `/tools/` bypass is therefore the load-bearing change** — it makes the SW pass tool requests straight through to Cloudflare's static serving.

solobase already exposes this as a consumer extension point: `sw.js.tmpl` has an `__EXTRA_BYPASS__` placeholder filled from `cfg.assets.extra_bypass_prefix` (read in `solobase/crates/solobase/src/cli/flows/embed_web.rs`). gizza sets these in `solobase.toml`. So the bypass is a **gizza config one-liner — no solobase code change.**

## Design — change set

All paths are under `gizza-ai/`.

### 1. SW bypass for `/tools/` (load-bearing)
- **`solobase.toml`** → `[assets].extra_bypass_prefix`: append `"/tools/"` to the existing list.
- Effect: the generated `sw.js` gains `|| url.pathname.startsWith('/tools/')` in its bypass clause, so `/tools/<slug>/index.html` and every sub-resource it loads (its own `*.js`/`*.wasm`/`tool.css`, all under `/tools/<slug>/`) are served statically without booting the runtime. One prefix covers all current and future tools.

### 2. Generator URLs → path form
- **`tools/generator/src/template.rs:10`** — `let canonical = format!("https://{}.gizza.ai/", meta.subdomain);` → `format!("https://gizza.ai/tools/{}/", meta.slug);` (keep the trailing slash; Pages serves `/tools/<slug>/` → `index.html`, so the canonical matches what's served and avoids a redirect-canonical mismatch). This `canonical` already feeds JSON-LD `url`, `<link rel=canonical>`, and `og:url`, so all three update together.
- **`tools/generator/src/seo.rs`** — `sitemap()` entry `https://{s}.gizza.ai/` → `https://gizza.ai/tools/{s}/`. `robots.txt` and the apex `<loc>https://gizza.ai/</loc>` line are unchanged.
- Update the unit-test assertions in both files (e.g. `template.rs:133` `https://calculator.gizza.ai/` → `https://gizza.ai/tools/calculator/`, and the `seo.rs` sitemap tests).

### 3. Rename `subdomain` → `slug`
The field is no longer a hostname; calling it `subdomain` is misleading (violates the workspace "no magic / misleading naming" rule). Rename across:
- **`tools/generator/src/meta.rs:20`** `pub subdomain: String,` → `pub slug: String,` (and its parse test assertions).
- **`tools/generator/src/template.rs`** every `meta.subdomain` → `meta.slug`.
- **`blocks/calculator/page/meta.toml:1`** and **`blocks/clock/page/meta.toml:1`** key `subdomain = "…"` → `slug = "…"`.
- Any `subdomain = "…"` literals inside the generator's inline test fixtures (`meta.rs`, `template.rs`).

### 4. Delete the subdomain machinery
- Delete **`functions/_middleware.js`**, **`functions/routing.mjs`**, **`functions/routing.test.mjs`**. With everything served at paths, the host→path rewrite has no remaining job; removing the Pages Function also drops a per-request Function invocation on the apex app. (Confirmed these three files are the entire `functions/` directory.)
- **`tests/tool_pages.spec.ts`** — repoint from "subdomain Host header rewrites to `/tools/<slug>`" to the path-based contract:
  - `GET /tools/calculator/` and `/tools/clock/` → 200 with the standalone page (correct `<title>`).
  - The SW bypass leaves tool pages static — assert no runtime boot (e.g. the page does not log `[gizza-ai] Loading WASM` / does not fetch the app's `gizza_ai_bg.wasm`).
  - `GET /` still serves the app shell.

### 5. Dashboard ops (manual, post-merge — user)
- In Cloudflare → Pages → `gizza-ai` → Custom domains, **remove `calculator.gizza.ai` and `clock.gizza.ai`** (added during the deploy bring-up; unused after this cutover). Keep `gizza.ai` and `www.gizza.ai`. No wildcard DNS, no Worker.

## Data flow (after cutover)

```
Browser → gizza.ai/tools/calculator/
  └─ runtime SW controlling the origin?
       └─ fetch handler: pathname startsWith '/tools/' → return (no respondWith)
            └─ Cloudflare Pages serves /tools/calculator/index.html
                 (static; loads only its own ~21–130K wasm; no solobase runtime)

Browser → gizza.ai/  (or any /b/… backend route)
  └─ SW intercepts (not bypassed) → handle_request → full app  (unchanged)
```

## Testing

- **Generator unit tests** (`cargo test` in `tools/generator`): canonical/JSON-LD/og + sitemap assert the `/tools/<slug>/` form; `meta` parse test asserts the `slug` field.
- **Playwright** (`tests/tool_pages.spec.ts`): path serving + SW-bypass (no runtime boot) + apex app still boots.
- **Post-deploy smoke** (real domain, after the deploy run is green):
  - `curl -sI https://gizza.ai/tools/calculator/` → 200; body `<title>Free Online Calculator — gizza.ai</title>` (already green pre-cutover via static serving).
  - In a browser that has already visited `gizza.ai` (so the runtime SW is installed + controlling): open `gizza.ai/tools/calculator/`, confirm DevTools shows the page served from static assets with **no** `[gizza-ai] Loading WASM…` console line and no `gizza_ai_bg.wasm` request.
  - `gizza.ai/` still loads the chat app.

## Risks & mitigations

- **SW propagation to returning visitors.** `sw.js` cache-busts via `?v=<sha>` on every deploy, so a visitor who already has the old (no-`/tools/`-bypass) SW gets the updated SW automatically on next visit/update — no manual unregister needed. Worst case for a split-second stale SW: the first tool-page hit is handled by the runtime (404/slow) until the SW updates; acceptable and self-healing, and irrelevant to first-time visitors who install the new SW immediately.
- **Forgotten bypass = silent regression.** If `/tools/` is omitted from `extra_bypass_prefix`, tool pages would be swallowed by the runtime in real browsers (but pass a naive `curl`, which never runs SWs). The Playwright "no runtime boot" assertion is the guard.
- **Pages trailing-slash behavior.** `gizza.ai/tools/calculator` (no slash) 308-redirects to `…/calculator/`; canonical uses the slash form to match. No action beyond using the trailing slash in the canonical.

## Rollout

Single gizza PR (branch + PR per workspace rules) containing changes 1–4. Merge to `main` triggers the existing Cloudflare Pages deploy. After it's green, do the dashboard cleanup (5). No solobase PR; no wafer-run PR.
