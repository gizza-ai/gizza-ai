# SEO/AI discoverability + shared chrome — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make gizza.ai discoverable by search engines + AI agents (`gizza list`-driven `sitemap.xml`/`robots.txt`/`llms.txt` + apex `<head>` SEO) and give it one shared, navigable header/footer (a `gizza-chrome` maud crate) across the chat app and the static tool pages.

**Architecture:** One source of truth per concern. A new leaf crate `chrome/` (`gizza-chrome`, maud-only) renders the header/footer/icons consumed by BOTH `src/blocks/ui.rs` (wasm chat) and `tools/generator/src/template.rs` (native pages). A new `scripts/gen-seo.sh` is the *only* writer of the root SEO files, driven by `gizza list --json-out` (the live registry), replacing the deleted `tools/generator/src/seo.rs`. The two projects share `ui.rs`, generator `main.rs`, and `solobase.toml`, so tasks are ordered to edit each shared file once or in sequence.

**Tech Stack:** Rust + maud 0.26, bash + jq, vanilla JS (ES modules, `node:test`), solobase/wafer-run runtime, GitHub Actions.

**Spec (read first, it holds the verbatim markup/copy this plan references):** `docs/superpowers/specs/2026-06-17-seo-discoverability-and-shared-chrome-design.md`.

## Global Constraints

- `BASE_URL` is the single constant `https://gizza.ai` (env-overridable in the script as `BASE_URL="${BASE_URL:-https://gizza.ai}"`). No other hardcoded host anywhere new.
- maud version `0.26`, `default-features = false`, everywhere (chrome crate + both consumers).
- The tool slug contract is `gizza list --json-out | jq -r '.[].name | sub("^gizza-ai/"; "")'`. `name = "gizza-ai/<slug>"`.
- Sitemap lists the apex `/` + only tools whose `blocks/<slug>/page/meta.toml` exists (no 404s).
- GitHub + Discord use their **official brand-mark SVGs** (Lucide deprecated brand icons); Lucide inline SVG for all functional/category icons. All icons are `Markup`-returning helpers in `gizza-chrome` (one copy).
- Do NOT change #85's behavior: `tools/generator/src/markdown.rs` `tool_markdown()`, the per-tool `index.md` write, and the `<link rel="alternate" type="text/markdown">` stay. This effort only *links into* `index.md` from `llms.txt`.
- Chat app gets a header but **no footer**; tool pages get both. Sitemap/robots/llms are written into `pkg/` root by the bash script (not overlaid from `site/`).
- Preserve every DOM hook `site/gizza-app.js` depends on (see Task 7).

---

## File structure

**New**
- `chrome/Cargo.toml`, `chrome/src/lib.rs` — `gizza-chrome`: `header(brand, active)`, `footer()`, icon helpers + unit tests.
- `scripts/gen-seo.sh`, `scripts/gen-seo.test.sh`
- `site/tools-index.js` (pure `filterTools`), `site/header.js`, `site/header.css`

**Edited**
- `Cargo.toml` (root) — `gizza-chrome = { path = "chrome" }`
- `tools/generator/Cargo.toml` — `gizza-chrome = { path = "../../chrome" }`
- `tools/generator/src/main.rs` — drop sitemap/robots writes + `mod seo` + slug collection; keep #85 `index.md` + `mod markdown`; copy header assets into each tool dir.
- `tools/generator/src/template.rs` — use shared header/footer; keep the #85 `rel=alternate`; link header assets.
- `tools/generator/src/markdown.rs` — drop the stale "matches `seo.rs`" comment only.
- `src/blocks/ui.rs` — SEO `<head>`; replace `sa-header` with shared header; load header assets; update tests.
- `site/gizza-app.js` — retarget the `sa-header h1` / `#open-settings` lookups.
- `site/tools-modal.js` — import `filterTools` from `tools-index.js`.
- `solobase.toml` — overlays + `extra_bypass_prefix` for `header.css`/`header.js`/`tools-index.js` + bypass for `/sitemap.xml`,`/robots.txt`,`/llms.txt`.
- `.github/workflows/deploy.yml`, `.github/workflows/test.yml`, `justfile`.

**Deleted**
- `tools/generator/src/seo.rs`

**Execution ordering (shared-file safety):** 1 → (2) → 3 → 4 → 5 → 6 → 7 → 8. `main.rs` is edited by Task 3 then Task 6; `ui.rs` by Task 4 then Task 7; `solobase.toml` only by Task 8. Tasks 1, 2, 5 are file-isolated.

---

### Task 1: `gizza-chrome` crate — header / footer / icons

**Files:**
- Create: `chrome/Cargo.toml`, `chrome/src/lib.rs`
- Test: inline `#[cfg(test)]` in `chrome/src/lib.rs`

**Interfaces:**
- Produces: `pub enum Active { Chat, Tool, None }`; `pub fn header(brand: maud::Markup, active: Active) -> maud::Markup`; `pub fn footer() -> maud::Markup`; icon helpers `pub fn icon_github() -> Markup`, `icon_discord()`, `icon_search()`, `icon_chevron_down()`, `icon_info()`, `icon_terminal()`, `icon_bot()` (all `-> Markup`). Header right side: an "Explore" mega-menu trigger (`#explore-trigger`, `.mega-menu`) with a Tools column (`<input type="search" id="tools-search">` + an empty `#tools-results` container `header.js` fills) and a Resources column (GitHub/Discord/CLI/SKILL.md/About links), plus standalone GitHub + Discord icon links. Use the exact layout/classes/copy from spec §"Project B — shared header + footer".

- [ ] **Step 1:** Create `chrome/Cargo.toml`: package `gizza-chrome`, edition 2021, one dep `maud = { version = "0.26", default-features = false }`. (No other deps — leaf crate.)
- [ ] **Step 2:** Write failing tests in `chrome/src/lib.rs` `#[cfg(test)]`:
```rust
#[test] fn header_has_brand_passthrough_and_nav() {
    let h = header(maud::html!{ span.brand-test { "BRANDX" } }, Active::Chat).into_string();
    assert!(h.contains("BRANDX"));                     // caller brand passed through
    assert!(h.contains("github.com"));                 // GitHub link
    assert!(h.contains("discord"));                    // Discord link
    assert!(h.contains("id=\"tools-search\""));        // Tools search input
    assert!(h.contains("id=\"tools-results\""));       // results container header.js fills
    assert!(h.contains("Explore"));                    // mega-menu trigger label
}
#[test] fn footer_has_blurb_and_columns() {
    let f = footer().into_string();
    assert!(f.contains("free") && f.contains("private")); // the existing blurb words
    assert!(f.contains("Tools") && f.contains("Resources"));
}
#[test] fn icons_return_svg() {
    for s in [icon_github(), icon_discord(), icon_search()] { assert!(s.into_string().contains("<svg")); }
}
```
- [ ] **Step 3:** Run `cargo test --manifest-path chrome/Cargo.toml` → FAIL (crate/fns missing).
- [ ] **Step 4:** Implement `Active`, `header`, `footer`, and the icon helpers using maud `html!`, following spec §"Project B" for layout/classes/copy and §"Icons" for the brand-mark vs Lucide split. GitHub/Discord = official brand-mark `<svg>` paths; functional icons = Lucide `<svg>`.
- [ ] **Step 5:** Run `cargo test --manifest-path chrome/Cargo.toml` → PASS.
- [ ] **Step 6:** Commit: `git add chrome/ && git commit -m "feat(chrome): gizza-chrome header/footer/icon crate"`.

---

### Task 2: `scripts/gen-seo.sh` + fixture test

**Files:**
- Create: `scripts/gen-seo.sh`, `scripts/gen-seo.test.sh`

**Interfaces:**
- Consumes: `$GIZZA list --json-out`, `blocks/<slug>/page/meta.toml` existence. Produces: `pkg/sitemap.xml`, `pkg/robots.txt`, `pkg/llms.txt`. Mirrors `scripts/scaffold-tool.test.sh`'s structure for the test.

- [ ] **Step 1:** Write `scripts/gen-seo.test.sh` (the failing test) following spec §"`scripts/gen-seo.test.sh`": create a temp dir with a stub `gizza` (a shell script emitting fixed `--json-out` JSON for e.g. `gizza-ai/calculator` (has page) + `gizza-ai/web-fetch` (chat-only)), fake `blocks/calculator/page/meta.toml`, no `blocks/web-fetch/page/meta.toml`; run `GIZZA=<stub> BASE_URL=https://example.test scripts/gen-seo.sh`; assert: `pkg/sitemap.xml` has `https://example.test/` and `https://example.test/tools/calculator/` but NOT `web-fetch`; `pkg/robots.txt` has `Sitemap: https://example.test/sitemap.xml`; `pkg/llms.txt` has `# gizza.ai`, a `calculator` line linking `…/tools/calculator/index.md`, and a `web-fetch` description line with no link.
- [ ] **Step 2:** Run `bash scripts/gen-seo.test.sh` → FAIL (`gen-seo.sh` missing).
- [ ] **Step 3:** Write `scripts/gen-seo.sh` per spec §"`scripts/gen-seo.sh`": `set -euo pipefail`; `BASE_URL`/`GIZZA` defaults; slug list via the contract; write the three files into `pkg/`. sitemap intersect with `blocks/<slug>/page/meta.toml`; robots constant body; llms.txt in llmstxt.org format (H1, `>` summary, `## Tools` with page-tool `index.md` links + chat-only descriptions, `## Resources`, closing `/tools/_index.json` + CLI pointer).
- [ ] **Step 4:** Run `bash scripts/gen-seo.test.sh` → PASS. `chmod +x` both scripts.
- [ ] **Step 5:** Commit: `git add scripts/gen-seo.sh scripts/gen-seo.test.sh && git commit -m "feat(seo): gizza list-driven sitemap/robots/llms.txt generator + test"`.

---

### Task 3: Remove the old generator SEO output

**Files:**
- Delete: `tools/generator/src/seo.rs`
- Modify: `tools/generator/src/main.rs` (remove `mod seo`, the sitemap/robots writes, and the slug collection feeding them), `tools/generator/src/markdown.rs` (drop the stale "matches the literal used in `seo.rs`" comment only — keep the `SITE` const).

**Interfaces:**
- Consumes: nothing. Produces: a generator that still emits pages + `_index.json` + #85 `index.md`, but no longer writes sitemap/robots.

- [ ] **Step 1:** `git rm tools/generator/src/seo.rs`.
- [ ] **Step 2:** In `main.rs`, delete `mod seo;`, the `seo::sitemap(...)`/`seo::robots(...)` write calls, and any slug `Vec` collected *only* for them (keep slugs still needed for `_index.json`/pages/`index.md`). In `markdown.rs`, delete only the stale `seo.rs` comment.
- [ ] **Step 3:** Run `cargo test --manifest-path tools/generator/Cargo.toml` → PASS (the `seo` unit tests are gone with the module; `index`/`meta`/`template`/`markdown` tests stay green — baseline was 15).
- [ ] **Step 4:** Commit: `git commit -am "refactor(generator): drop seo.rs — gen-seo.sh is the sole SEO writer"`.

---

### Task 4: Apex `<head>` SEO gap-fill (`src/blocks/ui.rs`)

**Files:**
- Modify: `src/blocks/ui.rs` (`render_chat()` `<head>`), and its `#[cfg(test)]` render tests.

**Interfaces:**
- Consumes: nothing. Produces: an apex `<head>` with title/description/canonical/OG/Twitter/JSON-LD. (Does NOT touch the header body — that's Task 7.)

- [ ] **Step 1:** Add a failing assertion to the existing `ui.rs` render test (or a new `#[test]`): the rendered chat HTML contains `rel="canonical" href="https://gizza.ai/"`, a `<meta name="description"`, `og:title`, `twitter:card`, and `"@type":"WebSite"`.
- [ ] **Step 2:** Run the `ui.rs` test → FAIL.
- [ ] **Step 3:** Implement the `<head>` additions per spec §"Apex `<head>` SEO gap-fill" (descriptive title, meta description, canonical, OG `type/title/description/url/image=https://gizza.ai/gis.png`, Twitter `summary`, JSON-LD `Organization`+`WebSite` with the same `</`-neutralization `template.rs` uses for its JSON-LD script).
- [ ] **Step 4:** Run the `ui.rs` test → PASS.
- [ ] **Step 5:** Commit: `git commit -am "feat(ui): apex chat <head> SEO (title/desc/canonical/OG/Twitter/JSON-LD)"`.

---

### Task 5: Shared client assets (`tools-index.js` / `header.js` / `header.css`)

**Files:**
- Create: `site/tools-index.js`, `site/header.js`, `site/header.css`
- Modify: `site/tools-modal.js` (import `filterTools` from `tools-index.js` instead of defining it)
- Test: a `node:test` for `filterTools` (e.g. `site/tools-index.test.mjs` if the repo runs node tests; otherwise assert via `gen-seo`-style check — match the repo's existing JS test convention, check `package.json`).

**Interfaces:**
- Produces: `export function filterTools(list, query)` in `tools-index.js`; `header.js` (mega-menu open/close + Tools search: fetch `/tools/_index.json` once, `filterTools`, render ≤~8 windowed rows into `#tools-results`, each linking `/tools/<slug>/`). Consumes: `header()`'s `#tools-search`/`#tools-results`/`#explore-trigger` ids from Task 1.

- [ ] **Step 1:** Write the failing `filterTools` test (same cases the current `tools-modal.js` relies on — empty query returns all, case-insensitive title/description/slug/tags match).
- [ ] **Step 2:** Run it → FAIL.
- [ ] **Step 3:** Create `site/tools-index.js` exporting `filterTools` (move the impl out of `tools-modal.js`); update `tools-modal.js` to `import { filterTools } from './tools-index.js'`. Create `site/header.js` (mega-menu + search wiring against Task 1's ids) and `site/header.css` (header/mega-menu/footer styling using the existing `--tool-accent` tokens).
- [ ] **Step 4:** Run the `filterTools` test → PASS; confirm `tools-modal.js` still works (no dup logic).
- [ ] **Step 5:** Commit: `git add site/tools-index.js site/header.js site/header.css site/tools-modal.js site/tools-index.test.mjs && git commit -m "feat(site): shared header.js/header.css + extract filterTools to tools-index.js"`.

---

### Task 6: Generator chrome integration (`template.rs` + `main.rs` + Cargo)

**Files:**
- Modify: `tools/generator/Cargo.toml` (`gizza-chrome = { path = "../../chrome" }`), `tools/generator/src/template.rs` (use `gizza_chrome::{header, footer}` with a static-logo `brand`; link `header.css`/`header.js`/`tools-index.js`; KEEP the #85 `rel=alternate`), `tools/generator/src/main.rs` (copy `site/header.css`/`header.js`/`tools-index.js` into each `pkg/tools/<slug>/`, same as `tool.js`/`tool.css`).
- Test: `template.rs` `#[cfg(test)]` updated.

**Interfaces:**
- Consumes: Task 1 `gizza_chrome::header/footer`; Task 5 asset filenames. Produces: tool pages rendered with the shared header/footer + asset links.

- [ ] **Step 1:** Update the `template` tests: assert the rendered page contains the shared header markers (`id="tools-search"`, `Explore`) and footer columns, AND still contains the #85 `rel="alternate" type="text/markdown"`.
- [ ] **Step 2:** Run `cargo test --manifest-path tools/generator/Cargo.toml` → FAIL.
- [ ] **Step 3:** Add the Cargo dep; in `template.rs` replace the thin logo nav/footer with `gizza_chrome::header(<static-logo brand>, Active::Tool)` + `gizza_chrome::footer()` and add the three asset `<link>`/`<script>`; in `main.rs` copy the three new assets into each tool dir.
- [ ] **Step 4:** Run `cargo test --manifest-path tools/generator/Cargo.toml` → PASS.
- [ ] **Step 5:** Commit: `git commit -am "feat(generator): render tool pages with shared gizza-chrome header/footer"`.

---

### Task 7: Chat-app chrome integration — replace `sa-header` (HIGH RISK)

**Files:**
- Modify: `Cargo.toml` (root, `gizza-chrome = { path = "chrome" }`), `src/blocks/ui.rs` (`render_chat()` body: replace the `sa-header` web component with `gizza_chrome::header(<mascot brand>, Active::Chat)`; load `header.css`/`header.js`/`tools-index.js`), `site/gizza-app.js` (retarget the brand/`#open-settings` lookups), and the `ui.rs` tests.

**Interfaces:**
- Consumes: Task 1 `gizza_chrome::header`; Task 5 assets. Produces: the chat shell with the shared header. **Preserve the exact DOM hooks `gizza-app.js` needs:** an `#open-settings` button (rendered in the shared header's actions area), the mascot markup (`.brand-mascot`, `#brand-still`, `#brand-video`, `.brand-eye*`) inside the `brand` block, and the `#composer-menu` items. Retarget `gizza-app.js`'s `document.querySelector('sa-header h1')` to the new brand wordmark selector.

- [ ] **Step 1:** Extend the `ui.rs` render test: shared header present (`id="tools-search"`), `#open-settings` still present, mascot hooks (`#brand-still`, `#brand-video`, `.brand-mascot`) intact, `sa-header` absent.
- [ ] **Step 2:** Run `ui.rs` test → FAIL.
- [ ] **Step 3:** Add the root Cargo dep; replace `sa-header` in `ui.rs` with the shared header (mascot as `brand`, keep `#open-settings` + mascot hooks); add the asset `<link>`/`<script>`; in `gizza-app.js` retarget the `sa-header h1` and `#open-settings` queries to the new selectors.
- [ ] **Step 4:** Run `ui.rs` test → PASS. (Manual chat smoke test is deferred to the post-deploy checklist — note it in the PR.)
- [ ] **Step 5:** Commit: `git commit -am "feat(ui): replace sa-header with shared gizza-chrome header"`.

---

### Task 8: Wiring — `solobase.toml` + CI + justfile + full verification

**Files:**
- Modify: `solobase.toml` (overlays + `extra_bypass_prefix` for `header.css`/`header.js`/`tools-index.js`; bypass for `/sitemap.xml`,`/robots.txt`,`/llms.txt`), `.github/workflows/deploy.yml` (build `gizza` CLI via `cargo install --path gizza-ai/cli` then run `scripts/gen-seo.sh`, working-directory `gizza-ai`, after "Build tool pages"), `.github/workflows/test.yml` (run `gen-seo.test.sh` + `cargo test --manifest-path chrome/Cargo.toml`), `justfile` (`seo` recipe: `GIZZA=cli/target/release/gizza scripts/gen-seo.sh`).

**Interfaces:** Consumes everything above. Produces: served SEO files + chat-delivered chrome assets + CI coverage.

- [ ] **Step 1:** Edit `solobase.toml` per spec §"Serve the root SEO files" + §"Shared client assets / Chat app" (overlays + the two bypass groups, matching the existing `tools-modal.js` pattern).
- [ ] **Step 2:** Edit `deploy.yml`, `test.yml`, `justfile` per spec §"CI / tooling".
- [ ] **Step 3:** Verify the toml/yaml parse and the targeted suites pass: `cargo test --manifest-path chrome/Cargo.toml`, `cargo test --manifest-path tools/generator/Cargo.toml`, `bash scripts/gen-seo.test.sh`, and `cargo test` for `ui.rs` (whichever manifest builds it cheapest). Note in the PR which heavier builds (full wasm `solobase build`) were/weren't run.
- [ ] **Step 4:** Commit: `git commit -am "chore(seo+chrome): serve SEO files, deliver header assets, CI + justfile"`.
- [ ] **Step 5:** Push `git push -u origin feat/seo-discoverability-and-chrome`; open a PR to `main` with the body listing what was tested, the deferred manual chat/browser smoke (model picker, send, tools modal, header dropdown, `/sitemap.xml`,`/robots.txt`,`/llms.txt`), and the `sa-header`-replacement risk note. Run `/code-review`. Do NOT merge.

---

## Self-Review

**Spec coverage:** Project A → Tasks 2 (gen-seo), 3 (delete seo.rs), 4 (apex head), 8 (solobase bypass + CI). Project B → Tasks 1 (chrome crate), 5 (assets), 6 (generator integration), 7 (chat integration), 8 (overlays). Shared chrome crate ✓ (1). filterTools dedupe ✓ (5). #85 preservation ✓ (constraints + 3/6). CI/justfile ✓ (8). All spec §sections map to a task.

**Placeholder scan:** Tasks cite the spec for verbatim markup/copy (header layout, `<head>` tags, llms.txt body) because the spec is in-repo and is the source of truth — implementers read both. Test code and interfaces are concrete. No "TBD/add error handling/similar-to" placeholders.

**Type consistency:** `header(brand: Markup, active: Active)`, `footer()`, `Active::{Chat,Tool,None}`, `filterTools(list, query)`, and the DOM ids (`#tools-search`, `#tools-results`, `#explore-trigger`, `#open-settings`, `#brand-still/#brand-video/.brand-mascot`) are named identically across Tasks 1, 5, 6, 7. Asset filenames `header.js`/`header.css`/`tools-index.js` consistent across 5/6/7/8.

**Shared-file ordering:** `main.rs` (3 before 6), `ui.rs` (4 before 7), `solobase.toml` (only 8), `gizza-chrome` (1 before 6/7) — all sequenced.
