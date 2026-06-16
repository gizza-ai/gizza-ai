# Design: searchable Tools modal (hammer button)

**Date:** 2026-06-16
**Repo:** gizza-ai (no solobase / wafer-run change)
**Builds on:** `2026-06-16-gizza-tool-pages-path-cutover-design.md` (tool pages now serve at `/tools/<slug>/`).

## Motivation

The home page currently renders every tool as an inline link in a bottom "Tools" section (`src/blocks/ui.rs`, fed by a `build.rs`-generated `TOOLS` const). That list is destined to grow to **thousands** of tools — inlining them all on every page load doesn't scale. Replace it with a **hammer button** beside the model ("brain") picker that opens a **searchable Tools modal**, loading the full list from a build-time static index fetched on demand.

## Goals

- A hammer icon button in the composer, immediately right of the brain/model button, opens a modal.
- The modal lists **all** tools (title + description), with a search box filtering on both, and each result navigates to `/tools/<slug>/`.
- Scales to thousands of tools with **zero added weight per page load** — the list is a static JSON index fetched on first open and cached.
- The old inline bottom "Tools" section and its `build.rs`/`TOOLS` plumbing are removed; the generator becomes the single source of the tool list.

## Non-goals (YAGNI)

- No category grouping, tags, favourites, or ranking — a flat title+description searchable list.
- No new metadata fields — `slug`, `title`, `description` already exist in `blocks/<tool>/page/meta.toml`.
- The modal lives only on the main app (where the composer/brain button is), not on the standalone tool pages.
- No server/runtime search — the index is static and search is client-side.

## Design

### A. Trigger — the hammer button
A new empty `<button id="open-tools" type="button" aria-label="Tools" title="Browse tools">` placed in the composer **immediately after `#open-brain-picker`** and before `#attach` (`src/blocks/ui.rs`). Its hammer glyph is drawn via a CSS `mask-image` `::before` rule in `tools-modal.css`, matching the existing icon-button convention used for the ⋮ (`#open-settings`) and brain (`#open-brain-picker`) buttons in `gizza.css`.

### B. Modal shell
A native `<dialog id="tools-modal">` added to the `ui.rs` markup, mirroring the existing About/Info `<dialog>` (gives ESC-to-close, `::backdrop`, and focus handling for free). Structure:
- header: a search `<input id="tools-search" type="search" placeholder="Search tools…">` + a close button (`<button value="close">` / form `method="dialog"`).
- a scrollable `<ul id="tools-results">` of result rows.

Each result row is `<li><a href="/tools/<slug>/"><span class="tool-title">title</span><span class="tool-desc">description</span></a></li>` — same-tab navigation. Behaviour, in `site/tools-modal.js`:
- **Open** (`#open-tools` click): `showModal()`, focus the search input, and ensure the index is loaded (see C).
- **Filter:** on `input`, case-insensitive substring match of the query against `title + " " + description`; re-render `#tools-results`. Empty query → show all.
- **States:** loading (while fetching), no-results ("No tools match …"), and fetch-error (message + a retry button that re-fetches).

### C. Data — static JSON index
The generator (`tools/generator`, `gizza-tool-pages`) already enumerates every tool (reads each `blocks/<tool>/page/meta.toml` into a `ToolMeta` with `slug`/`title`/`description`) and writes per-tool pages + `pkg/sitemap.xml` + `pkg/robots.txt`. Add one more output: **`pkg/tools/_index.json`** —

```json
[
  { "slug": "calculator", "title": "Free Online Calculator — gizza.ai", "description": "Evaluate expressions instantly." },
  { "slug": "clock", "title": "Current UTC Time — gizza.ai", "description": "…" }
]
```

It lives under `/tools/`, so it is **already covered by the `/tools/` Service Worker bypass** (from the path-cutover) — the SW serves it statically (no runtime boot), and the browser caches it. No new `solobase.toml` `extra_bypass_prefix` entry is needed for the index. `tools-modal.js` fetches `/tools/_index.json` **once on first open**, holds the parsed array in memory, and filters it client-side on every keystroke.

Path note: the leading underscore (`_index.json`) avoids colliding with a tool whose slug is `index`. Cloudflare Pages' special `_`-prefixed files (`_headers`, `_redirects`, `_routes.json`, `_worker.js`) are root-only, so a `_index.json` under `/tools/` is an ordinary static asset.

```
Build:  generator → pkg/tools/_index.json   [{slug,title,description} × thousands]
Open:   #open-tools click → dialog.showModal() → (first open) fetch('/tools/_index.json') → cache
Type:   filter cached array on title+description → render <a href="/tools/{slug}/">
```

### D. Remove the dead bottom list + its plumbing
- `src/blocks/ui.rs`: delete the `@if !TOOLS.is_empty() { section.gizza-tools … }` block and the `include!(…"/tools.rs")` line.
- `build.rs`: delete the tools-scanning (the `tools` vec, the `page/meta.toml` `toml_str_value` reads, the `tools.rs` generation). **Keep** the `SKILLS` scanning (the block-wasm embedding) untouched.
- Delete the `renders_tools_interlink` test in `ui.rs` (replaced by the modal test in F).

This removes the second, parallel tool-list scanner (`build.rs`), leaving the generator as the single source of the tool list (pages + sitemap + index).

### E. New / changed files (all under `gizza-ai/`)
- **New** `site/tools-modal.js` — the modal controller + a pure exported `filterTools(list, query)` (for unit testing).
- **New** `site/tools-modal.css` — the hammer icon mask + modal/list/search styling.
- **`solobase.toml`** — add `site/tools-modal.js` → `tools-modal.js` and `site/tools-modal.css` → `tools-modal.css` as `[[assets.overlay]]` entries, and add `"/tools-modal.js"`, `"/tools-modal.css"` to `extra_bypass_prefix` (exactly like the existing `model-picker.js/css`). (The `/tools/_index.json` index needs no bypass entry — `/tools/` already covers it.)
- **`tools/generator`** — emit `pkg/tools/_index.json` (serialize `[{slug,title,description}]` via `serde_json`; the crate already depends on `serde_json`).
- **`src/blocks/ui.rs`** — add the `#open-tools` button, the `<dialog id="tools-modal">`, the `<link rel="stylesheet" href="/tools-modal.css">` and `<script type="module" src="/tools-modal.js">` tags; remove the `gizza-tools` section + `TOOLS` include.
- **`build.rs`** — remove tools-scanning / `tools.rs` generation.

### F. Testing
- **Generator unit test:** `_index.json` content is valid JSON, is an array, and each entry has `slug`/`title`/`description`; the calculator/clock entries are present with their descriptions.
- **`site/tools-modal.js` unit test** (`js/tools-modal.test.js`, ESM `node:test`, like the existing `js/*.test.js`): `filterTools` — query matches the title, matches the description, is case-insensitive, empty query returns the full list, no-match returns `[]`.
- **`ui.rs` test:** the rendered home page contains `id="open-tools"` (hammer button) and `id="tools-modal"` (the dialog), and no longer contains `class="gizza-tools"`.

### G. Verification (post-deploy)
- `curl -s https://gizza.ai/tools/_index.json | head` → JSON array with the tools.
- In the app: the hammer appears beside the brain button; clicking opens the modal; typing filters; clicking a result lands on `/tools/<slug>/`. The bottom "Tools" list is gone.

## Risks & mitigations

- **Index staleness across deploys:** the index is regenerated every build from the same `meta.toml` files that produce the pages, so it can't drift from the pages. The SW cache-busts on deploy (existing mechanism), and a static JSON under `/tools/` is revalidated by normal HTTP caching; acceptable for a tool directory.
- **Index path special-casing on Pages:** mitigated by the `_index.json`-under-`/tools/` choice (Pages `_`-special files are root-only).
- **Fetch failure / offline:** the modal shows an error state with a retry button rather than a blank list.
- **Large index payload:** thousands × `{slug,title,description}` is on the order of a few hundred KB of JSON, fetched once and cached — only when the user opens the modal. If it ever grows large enough to matter, a follow-up could paginate or trim fields, but that's out of scope now.

## Rollout

Single gizza PR (branch + PR per workspace rules). Merge triggers the existing Cloudflare Pages deploy. No solobase / wafer-run change.
