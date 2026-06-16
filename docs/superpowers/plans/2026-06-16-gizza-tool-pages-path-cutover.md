# Tool Pages Path Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve gizza's standalone tool pages at `https://gizza.ai/tools/<slug>/` instead of per-tool subdomains, and remove the dead subdomain routing.

**Architecture:** Tool pages are already built as lightweight static pages at `pkg/tools/<slug>/` by the `gizza-tool-pages` generator. The cutover (1) adds `/tools/` to the runtime Service Worker's bypass list so the SW serves those paths statically instead of booting the wafer/solobase runtime, (2) repoints the generator's canonical/OpenGraph/sitemap URLs to the path form, (3) renames the misnamed `subdomain` field to `slug`, and (4) deletes the Cloudflare Pages subdomain middleware. No solobase or wafer-run change.

**Tech Stack:** Rust (the `gizza-tool-pages` generator, a standalone crate under `tools/generator`), `solobase build` (wasm-pack + asset bundling, driven by `solobase.toml`), Node `node:test` (JS unit tests), GitHub Actions.

**Branch:** `feat/tool-pages-path-cutover` (already created; the design spec is already committed there). All task commits land on this branch; Task 7 opens the PR.

**Reference spec:** `docs/superpowers/specs/2026-06-16-gizza-tool-pages-path-cutover-design.md`

---

### Task 1: Rename `subdomain` → `slug` across the generator (no behavior change)

The field is a path segment now, not a hostname. This is a mechanical rename; all existing tests stay green because URLs are unchanged (a `slug` of `"calculator"` still formats to `https://calculator.gizza.ai/` until Task 2). Doing the rename first keeps later tasks readable.

**Files:**
- Modify: `tools/generator/src/meta.rs`
- Modify: `tools/generator/src/template.rs`
- Modify: `tools/generator/src/seo.rs`
- Modify: `tools/generator/src/main.rs`
- Modify: `blocks/calculator/page/meta.toml`
- Modify: `blocks/clock/page/meta.toml`

- [ ] **Step 1: Rename in the four Rust source files**

In each of `tools/generator/src/{meta.rs,template.rs,seo.rs,main.rs}`, replace every occurrence of the identifier/word `subdomain` with `slug`. Concretely this changes:
- `meta.rs`: the struct field `pub subdomain: String,` → `pub slug: String,`; the three inline test fixtures `subdomain     = "calculator"` / `subdomain     = "clock"` → `slug          = "…"`; and the assertion `assert_eq!(m.subdomain, "calculator");` → `assert_eq!(m.slug, "calculator");`.
- `template.rs`: line 10 `format!("https://{}.gizza.ai/", meta.subdomain)` → `format!("https://{}.gizza.ai/", meta.slug)` (URL format unchanged for now); the inline test fixture `subdomain     = "calculator"` → `slug          = "calculator"`.
- `seo.rs`: the parameter `pub fn sitemap(subdomains: &[String])` → `pub fn sitemap(slugs: &[String])`; the loop `for s in subdomains` → `for s in slugs`; the doc comment "every tool subdomain" → "every tool slug" (the test name `sitemap_lists_apex_and_subdomains` may stay or become `..._and_slugs` — rename it for clarity).
- `main.rs`: `pkg_tools.join(&m.subdomain)` → `…join(&m.slug)`; the two error/log strings `m.subdomain` → `m.slug`; `let subdomains: Vec<String> = … m.subdomain.clone()` → `let slugs: Vec<String> = … m.slug.clone()`; `seo::sitemap(&subdomains)` → `seo::sitemap(&slugs)`.

- [ ] **Step 2: Rename the TOML key in both tool meta files**

`blocks/calculator/page/meta.toml` line 1: `subdomain     = "calculator"` → `slug          = "calculator"`
`blocks/clock/page/meta.toml` line 1: `subdomain     = "clock"` → `slug          = "clock"`

(The serde struct now expects the key `slug`; leaving these as `subdomain` would make `ToolMeta::from_toml` fail at build time.)

- [ ] **Step 3: Run the generator tests to verify the rename compiles and stays green**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — all existing tests still pass (URLs unchanged; only the identifier changed). If you see `no field 'subdomain'` or `missing field 'slug'`, you missed an occurrence — grep `rg -w subdomain tools/generator blocks/*/page/meta.toml` and finish the rename.

- [ ] **Step 4: Commit**

```bash
git add tools/generator/src blocks/calculator/page/meta.toml blocks/clock/page/meta.toml
git commit -m "refactor(gizza-tool-pages): rename subdomain field to slug

It is a URL path segment now, not a hostname. Mechanical rename; URLs and
behavior unchanged."
```

---

### Task 2: Repoint canonical / OpenGraph URL to `/tools/<slug>/`

**Files:**
- Modify: `tools/generator/src/template.rs:10` (the `canonical` binding) and its test assertion (~line 133)

- [ ] **Step 1: Update the failing test assertion**

In `tools/generator/src/template.rs`, in `fn includes_seo_head_and_widget`, change:

```rust
        assert!(html.contains(r#"<link rel="canonical" href="https://calculator.gizza.ai/">"#));
```

to:

```rust
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/calculator/">"#));
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml includes_seo_head_and_widget`
Expected: FAIL — the rendered HTML still contains `https://calculator.gizza.ai/`, not the new path form.

- [ ] **Step 3: Change the canonical URL**

In `tools/generator/src/template.rs`, change line 10 from:

```rust
    let canonical = format!("https://{}.gizza.ai/", meta.slug);
```

to:

```rust
    let canonical = format!("https://gizza.ai/tools/{}/", meta.slug);
```

(The `canonical` value feeds JSON-LD `"url"`, `<link rel="canonical">`, and `<meta property="og:url">`, so all three update from this one change. Keep the trailing slash: Cloudflare Pages serves `/tools/<slug>/` → `index.html`, so the canonical matches what is served.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — all generator tests green.

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/template.rs
git commit -m "feat(gizza-tool-pages): canonical/og URL → https://gizza.ai/tools/<slug>/"
```

---

### Task 3: Repoint sitemap URLs to `/tools/<slug>/`

**Files:**
- Modify: `tools/generator/src/seo.rs` (the `sitemap` body + its test)

- [ ] **Step 1: Update the failing test assertions**

In `tools/generator/src/seo.rs`, in the sitemap test, change the two per-tool assertions:

```rust
        assert!(xml.contains("<loc>https://calculator.gizza.ai/</loc>"));
        assert!(xml.contains("<loc>https://clock.gizza.ai/</loc>"));
```

to:

```rust
        assert!(xml.contains("<loc>https://gizza.ai/tools/calculator/</loc>"));
        assert!(xml.contains("<loc>https://gizza.ai/tools/clock/</loc>"));
```

(Leave the apex assertion `assert!(xml.contains("<loc>https://gizza.ai/</loc>"));` unchanged.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml -- seo`
Expected: FAIL — sitemap still emits `https://<slug>.gizza.ai/`.

- [ ] **Step 3: Change the sitemap URL format**

In `tools/generator/src/seo.rs`, change the loop body:

```rust
        urls.push_str(&format!("  <url><loc>https://{s}.gizza.ai/</loc></url>\n"));
```

to:

```rust
        urls.push_str(&format!("  <url><loc>https://gizza.ai/tools/{s}/</loc></url>\n"));
```

(Update the `sitemap` doc comment to say "every tool page under /tools/" instead of "every tool subdomain".)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/seo.rs
git commit -m "feat(gizza-tool-pages): sitemap URLs → https://gizza.ai/tools/<slug>/"
```

---

### Task 4: Add `/tools/` to the Service Worker bypass (the load-bearing change)

Without this, on the apex origin the root-scoped runtime SW (`sw.js`) intercepts `/tools/<slug>/` and runs it through the wasm runtime instead of serving the static page. solobase injects bypass prefixes into `sw.js` from `solobase.toml`'s `[assets].extra_bypass_prefix` via the `__EXTRA_BYPASS__` placeholder, so this is a gizza config change plus a guard test.

**Files:**
- Modify: `solobase.toml` (`[assets].extra_bypass_prefix`)
- Create: `js/sw-bypass.test.js`

- [ ] **Step 1: Write the failing guard test**

Create `js/sw-bypass.test.js`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// The runtime Service Worker (sw.js) must NOT intercept /tools/* — those are
// lightweight standalone pages that must be served as static assets, never
// booted through the wafer/solobase runtime. solobase injects this bypass from
// solobase.toml [assets].extra_bypass_prefix into sw.js via __EXTRA_BYPASS__.
// Requires `solobase build` to have produced pkg/sw.js (CI builds before tests).
const swPath = fileURLToPath(new URL('../pkg/sw.js', import.meta.url));

test('sw.js bypasses /tools/ so tool pages stay static', () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/tools\/['"]\)/,
    'sw.js is missing the /tools/ bypass — check extra_bypass_prefix in solobase.toml',
  );
});
```

- [ ] **Step 2: Build the site, then run the test to verify it fails**

Run:
```bash
solobase build            # generates pkg/sw.js (debug is fine here; faster than --release)
node --test js/sw-bypass.test.js
```
Expected: FAIL — `pkg/sw.js` does not yet contain `startsWith('/tools/')` (the bypass isn't configured).

- [ ] **Step 3: Add `/tools/` to the bypass list**

In `solobase.toml`, in the `[assets]` table, append `"/tools/"` to the `extra_bypass_prefix` array (end of the existing list):

```toml
extra_bypass_prefix = ["/gizza-app.js", "/gizza.css", "/render.js", "/pending.js", "/gis.png", "/gis_no_eyes.png", "/gis_a_job_no_eyes.png", "/eye.png", "/gis_video_idle.mp4", "/gis_video_typing_loop.mp4", "/gis_video_typing_finish.mp4", "/favicon.ico", "/favicon-32.png", "/apple-touch-icon.png", "/model-picker.js", "/model-picker.css", "/tool.js", "/tool.css", "/tools/"]
```

- [ ] **Step 4: Rebuild and run the test to verify it passes**

Run:
```bash
solobase build
node --test js/sw-bypass.test.js
```
Expected: PASS — `pkg/sw.js` now contains `url.pathname.startsWith('/tools/')`.

- [ ] **Step 5: Commit**

```bash
git add solobase.toml js/sw-bypass.test.js
git commit -m "feat(gizza): bypass /tools/ in the runtime Service Worker

Tool pages are served at /tools/<slug>/ on the apex origin now, so the
root-scoped runtime SW must pass them through to static serving instead of
booting the wasm runtime. Guarded by js/sw-bypass.test.js."
```

---

### Task 5: Delete the subdomain machinery

With everything served at paths, the Cloudflare Pages host→path middleware has no job. The existing Playwright spec (`tests/tool_pages.spec.ts`) already tests path-based serving (`/tools/calculator/`), so it stays — only its stale comment that references the deleted node test needs fixing.

**Files:**
- Delete: `functions/_middleware.js`, `functions/routing.mjs`, `functions/routing.test.mjs`
- Modify: `tests/tool_pages.spec.ts` (stale comment only)

- [ ] **Step 1: Delete the three subdomain-routing files**

```bash
git rm functions/_middleware.js functions/routing.mjs functions/routing.test.mjs
```

- [ ] **Step 2: Fix the stale comment in the Playwright spec**

In `tests/tool_pages.spec.ts`, the header comment ends with:

```
// /tools/<sub>/. The *.gizza.ai subdomain rewrite is covered by
// functions/routing.test.mjs (node test), not here.
```

Replace those two lines with:

```
// /tools/<slug>/. Tool pages are served at apex paths (no subdomains); the
// runtime Service Worker bypasses /tools/ (see js/sw-bypass.test.js).
```

- [ ] **Step 3: Verify the JS unit tests still pass**

Run:
```bash
npm install --no-audit --no-fund
npm test
```
Expected: PASS — `node --test js/*.test.js` runs the existing js tests plus the new `js/sw-bypass.test.js` (requires `pkg/sw.js` from the Task 4 build). Deleting `functions/*` does not affect `npm test` (it never globbed `functions/`).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(gizza): delete dead subdomain Pages middleware

Tools are served at /tools/<slug> paths; the host→path rewrite and its node
test are obsolete. Removes a per-request Pages Function on the apex app."
```

---

### Task 6: Repoint the app's tool list — `build.rs` + `ui.rs` (the missed consumers)

The `subdomain` meta key has two readers outside the generator. After Task 1 renamed the key to `slug`, `build.rs` reads an absent key → the generated `TOOLS` const is empty → the home-page "Tools" section disappears and its test fails. This task fixes both readers and repoints the link to the path form. (This corrects a wrong assumption in the original spec; see the spec's "Correction" section. The `renders_tools_interlink` test currently FAILS on this branch.)

**Files:**
- Modify: `build.rs` (reads `page/meta.toml`, generates `TOOLS`)
- Modify: `src/blocks/ui.rs` (renders the Tools section + its test `renders_tools_interlink`)
- Modify (comments only): `blocks/calculator/web/src/lib.rs`, `blocks/clock/src/lib.rs`, `site/tool.js`

- [ ] **Step 1: Update the failing test to assert path URLs**

In `src/blocks/ui.rs`, in `fn renders_tools_interlink` (~line 296), change:

```rust
        assert!(s.contains("https://calculator.gizza.ai"), "calculator link present");
        assert!(s.contains("https://clock.gizza.ai"), "clock link present");
```

to:

```rust
        assert!(s.contains("/tools/calculator/"), "calculator link present");
        assert!(s.contains("/tools/clock/"), "clock link present");
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test renders_tools_interlink`
Expected: FAIL — currently it panics at `"tools section present"` because `TOOLS` is empty (build.rs reads the now-absent `subdomain` key).

- [ ] **Step 3: Fix `build.rs` to read the `slug` key**

In `build.rs`, change the meta read (~line 44–48) from:

```rust
                if let (Some(subdomain), Some(h1)) = (
                    toml_str_value(&meta_text, "subdomain"),
                    toml_str_value(&meta_text, "h1"),
                ) {
                    tools.push((subdomain, h1));
                }
```

to:

```rust
                if let (Some(slug), Some(h1)) = (
                    toml_str_value(&meta_text, "slug"),
                    toml_str_value(&meta_text, "h1"),
                ) {
                    tools.push((slug, h1));
                }
```

Also rename the `subdomain` wording in `build.rs` comments/loop vars to `slug` for accuracy: line 29 `// (subdomain, title)` → `// (slug, title)`; line 82 `// Sort tools by subdomain` → `// Sort tools by slug`; line 104 generated-comment `// (subdomain, title) for every tool…` → `// (slug, title) for every tool…`; line 106 `for (subdomain, title) in &tools` → `for (slug, title) in &tools`; line 108 `({subdomain:?}, …)` → `({slug:?}, …)`.

- [ ] **Step 4: Fix `ui.rs` to link to the path form**

In `src/blocks/ui.rs` (~line 152–154), change:

```rust
                            @for (sub, title) in TOOLS {
                                li {
                                    a href=(format!("https://{sub}.gizza.ai")) { (title) }
                                }
```

to:

```rust
                            @for (slug, title) in TOOLS {
                                li {
                                    a href=(format!("/tools/{slug}/")) { (title) }
                                }
```

(Relative apex path; works on any host, and the SW `/tools/` bypass passes it through.)

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cargo test renders_tools_interlink`
Expected: PASS — `TOOLS` is populated (build.rs reads `slug`) and the rendered links are `/tools/calculator/`, `/tools/clock/`.

- [ ] **Step 6: Reword stale doc comments (cosmetic)**

- `blocks/calculator/web/src/lib.rs:2`: `//! Compiled with wasm-pack for the standalone calculator.gizza.ai page.` → `//! Compiled with wasm-pack for the standalone /tools/calculator/ page.`
- `blocks/clock/src/lib.rs:4`: reword `standalone clock.gizza.ai page` → `standalone /tools/clock/ page`.
- `site/tool.js:3`: `Shared by every tool subdomain.` → `Shared by every tool page (/tools/<slug>/).`

- [ ] **Step 7: Run the full lib test suite and commit**

Run: `cargo test --lib`
Expected: PASS (incl. `renders_tools_interlink`).

```bash
git add build.rs src/blocks/ui.rs blocks/calculator/web/src/lib.rs blocks/clock/src/lib.rs site/tool.js
git commit -m "feat(gizza): repoint app tool list to /tools/<slug> paths

build.rs now reads the renamed slug key (else TOOLS is empty); the home-page
Tools section links to /tools/<slug>/ instead of <slug>.gizza.ai. Reword stale
subdomain comments."
```

---

### Task 7: Run the generator's unit tests in CI

`tools/generator` is a standalone crate (`gizza-tool-pages`), not a workspace member, so the existing `cargo test` step does not run it — the URL/slug guards from Tasks 1–3 would not gate. Add an explicit step.

**Files:**
- Modify: `.github/workflows/test.yml`

- [ ] **Step 1: Add a generator-tests step after the existing "Cargo tests" step**

In `.github/workflows/test.yml`, immediately after the `Cargo tests` step (the one running `cargo test` in `working-directory: gizza-ai`), add:

```yaml
      - name: Tool-page generator tests
        working-directory: gizza-ai
        run: cargo test --manifest-path tools/generator/Cargo.toml
```

- [ ] **Step 2: Verify the command locally**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — same command CI will run; confirms it's green.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/test.yml
git commit -m "ci(gizza): run the gizza-tool-pages generator unit tests"
```

---

### Task 8: Push and open the PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feat/tool-pages-path-cutover
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create -R gizza-ai/gizza-ai --base main --head feat/tool-pages-path-cutover \
  --title "Serve tool pages at /tools/<slug> instead of subdomains" \
  --body "Implements docs/superpowers/specs/2026-06-16-gizza-tool-pages-path-cutover-design.md.

Cloudflare Pages can't do wildcard custom domains, so per-tool subdomains don't scale. Serve tools at apex paths (gizza.ai/tools/<slug>) instead: pure static, free, no per-tool DNS/Worker, and SEO authority pools into one domain.

- SW bypass /tools/ (solobase.toml) so the runtime SW leaves tool pages static — guarded by js/sw-bypass.test.js
- generator canonical/og/sitemap → /tools/<slug>/; rename subdomain field → slug
- app home-page Tools section links → /tools/<slug>/ (build.rs reads slug; ui.rs path link)
- delete the dead functions/ subdomain middleware
- run the generator unit tests in CI

No solobase / wafer-run change.

After merge (manual, CF dashboard): remove the calculator.gizza.ai and clock.gizza.ai custom domains from the gizza-ai Pages project; keep gizza.ai + www.

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

- [ ] **Step 3: Wait for CI green, then report**

Run: `gh pr checks <PR#> -R gizza-ai/gizza-ai --watch`
Expected: the `test` workflow passes (cargo tests, generator tests, JS tests incl. sw-bypass). Report the PR URL and CI status; do not merge without user sign-off.

---

## Post-merge manual step (user, Cloudflare dashboard)

Not code — track separately. In Cloudflare → Pages → `gizza-ai` → Custom domains, remove `calculator.gizza.ai` and `clock.gizza.ai` (added during deploy bring-up; unused after this cutover). Keep `gizza.ai` and `www.gizza.ai`. No wildcard DNS or Worker is needed.

## Verification summary (after deploy)

- `curl -sI https://gizza.ai/tools/calculator/` → 200; body title `Free Online Calculator — gizza.ai`.
- In a browser that has already loaded `gizza.ai` (runtime SW installed + controlling): open `gizza.ai/tools/calculator/` → DevTools shows it served from static assets, **no** `[gizza-ai] Loading WASM…` console line, no `gizza_ai_bg.wasm` request.
- `https://gizza.ai/` still boots the chat app.
