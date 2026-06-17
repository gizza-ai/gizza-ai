# SEO/AI discoverability + shared site chrome — design

**Date:** 2026-06-17
**Status:** approved (design); ready for implementation plan
**Repo:** `gizza-ai`

## Goal

Make gizza.ai discoverable by both search engines and AI agents, and give it a
consistent, navigable site chrome (header + footer) across its two rendered
surfaces. Two coupled efforts, shipped together:

- **Project A — discoverability:** a `gizza list`-driven generator for
  `sitemap.xml`, `robots.txt`, and `llms.txt`, plus filling the SEO gaps in the
  apex chat page's `<head>`.
- **Project B — chrome:** one shared header (logo + GitHub + Discord + an
  "Explore" mega-menu) and footer, used by both the chat app and the static
  tool pages. The mega-menu's Tools column is a **search** over the tools index
  (the tool catalog is heading into the thousands), not a static list.

## Background — the two surfaces

gizza.ai renders HTML two ways, both via **maud**:

1. **Apex `/` — the chat app** (`src/blocks/ui.rs`), compiled to wasm and served
   by the runtime. Today its header is the external `sa-header` web component
   (`site-kit.suppers.ai`); `gizza-app.js` queries it (`sa-header h1`) and moves
   the `#open-settings` button into the composer. Its `<head>` is SEO-bare:
   title is just `"gizza-ai"`, with no description/canonical/OG/Twitter/JSON-LD.
2. **`/tools/<slug>/` — standalone tool pages** (`tools/generator/src/template.rs`),
   rendered by the native `gizza-tool-pages` binary from `blocks/<slug>/page/meta.toml`.
   These already have good SEO (`title`, description, canonical, OG, Twitter,
   JSON-LD `WebApplication`) and a thin logo nav + footer.

`tools/generator` is its **own** single-package workspace with only
`maud`/`serde`/`toml`/`pulldown-cmark` — it deliberately does not depend on the
heavy chat lib. That constraint shapes the shared-chrome approach below.

`gizza list` (the `cli/` crate's `gizza` binary) boots the wasmi runtime and
lists every registered skill tool — **14 today** (calculator, clock, ffmpeg,
image-convert, image-crop, image-fetch, image-grayscale, image-resize, imagine,
video-frame-extract, video-transcode, video-trim, web-fetch, word-count). Only
**5** have a standalone page (calculator, clock, image-grayscale, image-resize,
word-count). `gizza list --json-out` emits `[{name,description,parameters}]`
with `name = "gizza-ai/<slug>"`.

The existing tools modal (`site/tools-modal.js`) already fetches
`/tools/_index.json` and filters client-side via a pure `filterTools(list, query)`.

## Non-goals

- No per-tool standalone pages for the 9 chat-only tools (sitemap lists only
  tools that have a real page → no 404s).
- No `llms-full.txt` (the machine-readable full catalog is `/tools/_index.json`
  + `gizza list`).
- No marketing pages (pricing/templates/etc. from the reference screenshot do
  not exist on gizza and are not being created).

## Architecture — one source of truth per concern

### Shared chrome crate

New leaf crate **`chrome/`** (package `gizza-chrome`), dependency: `maud` only
(`default-features = false`, version `0.26` to match both consumers). It exports:

- `pub fn header(brand: Markup, active: Active) -> Markup`
- `pub fn footer() -> Markup`
- inline-SVG icon helpers (Lucide functional/category icons + GitHub/Discord
  brand marks) as `Markup`-returning fns.

`brand` is passed in by the caller so each surface supplies its own brand block
(the chat app's animated mascot vs. the tool page's static logo) while the
right-hand nav, mega-menu, and footer markup stay identical. `Active` marks the
current section for highlighting (e.g. `Chat`, `Tool`).

Both consumers add a path dep:
- root `Cargo.toml`: `gizza-chrome = { path = "chrome" }`
- `tools/generator/Cargo.toml`: `gizza-chrome = { path = "../../chrome" }`

The crate is a leaf of static markup, so it adds negligible weight to the
size-budgeted wasm bundle.

### Shared client assets

Three new files under `site/`, served on **both** surfaces:

- **`site/tools-index.js`** — the pure search core: `export function filterTools(list, query)`
  (moved here from `tools-modal.js`, which now imports it; dedupes the logic).
- **`site/header.js`** — header behavior: open/close the mega-menu, and power the
  Tools search (fetch `/tools/_index.json` once, `filterTools`, render a capped
  windowed result list). Imports `filterTools` from `tools-index.js`.
- **`site/header.css`** — header + mega-menu + footer styling, matching the
  existing design tokens (`--tool-accent` etc.).

Delivery to each surface:
- **Chat app:** added to `solobase.toml` `[[assets.overlay]]` + `extra_bypass_prefix`
  (same pattern as `tools-modal.js`/`.css`), and `<link>`/`<script>` in `ui.rs`.
- **Tool pages:** the generator copies them into each `pkg/tools/<slug>/` (same
  as today's `tool.js`/`tool.css`), and `template.rs` references them.

## Project A — SEO / AI discoverability

### `scripts/gen-seo.sh` (new)

Single generator, driven by `gizza list`. `set -euo pipefail`. Config:
`BASE_URL="${BASE_URL:-https://gizza.ai}"` (one constant, env-overridable);
`GIZZA="${GIZZA:-gizza}"` (so local runs can point at `cli/target/release/gizza`).
Run from repo root. Writes three files into `pkg/`:

Slugs come from the stable JSON contract:
```sh
$GIZZA list --json-out | jq -r '.[].name | sub("^gizza-ai/"; "")' | sort
```

1. **`pkg/sitemap.xml`** — apex `<loc>$BASE_URL/</loc>` plus
   `<loc>$BASE_URL/tools/<slug>/</loc>` for each slug **where
   `blocks/<slug>/page/meta.toml` exists** (the intersection → no 404s). Scales
   to thousands within the 50,000-URL sitemap limit.
2. **`pkg/robots.txt`** — constant body:
   ```
   User-agent: *
   Allow: /
   Sitemap: $BASE_URL/sitemap.xml
   ```
3. **`pkg/llms.txt`** — [llmstxt.org](https://llmstxt.org) format:
   - `# gizza.ai`
   - a `>` summary blockquote (browser-local AI chat + tools; private; CLI).
   - `## Tools` — one bullet per tool from `gizza list`
     (`- [name](url): description` when a page exists; `- name: description`
     otherwise), so it stays drift-free.
   - `## Resources` — GitHub, Discord, CLI README, SKILL.md.
   - a closing note pointing AI agents at `/tools/_index.json` and the `gizza`
     CLI as the authoritative machine-readable catalog (keeps llms.txt an
     overview as the tool count grows into the thousands).

### Remove the old generator SEO output

`tools/generator/src/seo.rs` is **deleted**, and the `sitemap`/`robots` writes
(+ `mod seo` + slug collection) are removed from `main.rs`. The bash script is
now the only writer of these files (eliminates the two-writer drift). The
generator keeps producing pages + `_index.json`.

### Apex `<head>` SEO gap-fill (`src/blocks/ui.rs`)

Add to `render_chat()`'s `<head>`: descriptive `<title>`, meta description,
`<link rel="canonical" href="https://gizza.ai/">`, OG (`type/title/description/url/image`,
image = `https://gizza.ai/gis.png`), Twitter `summary` card, and JSON-LD
`Organization` + `WebSite`. Same `</`-neutralization for the JSON-LD `<script>`
as `template.rs` already uses.

### CI / tooling

- **`.github/workflows/deploy.yml`:** after "Build tool pages", add a step that
  installs the gizza CLI (`cargo install --path gizza-ai/cli` — no `--locked`;
  the lockfile is gitignored) and runs `scripts/gen-seo.sh` (working-directory
  `gizza-ai`). The `block.wasm` files the CLI embeds already exist after the
  "Build site" step, so this is buildable here.
- **`scripts/gen-seo.test.sh`** (new, mirrors `scripts/scaffold-tool.test.sh`):
  runs the script against a fixture (a stub `gizza` emitting known JSON + fake
  `blocks/*/page/meta.toml`) and asserts: apex present; a page-tool's URL
  present in the sitemap; a chat-only tool's URL **absent**; robots has the
  `Sitemap:` line; llms.txt has the H1, a page-tool link, and a chat-only tool's
  description line.
- **`.github/workflows/test.yml`:** run `gen-seo.test.sh` and `cargo test` for
  the `chrome` crate; the generator's remaining `cargo test` covers
  `index`/`meta`/`template` (the `seo` unit tests are gone with the module).
- **`justfile`:** add a `seo` recipe (`GIZZA=cli/target/release/gizza scripts/gen-seo.sh`).

## Project B — shared header + footer

### Header (`gizza-chrome::header`), both surfaces

Layout: **left** = brand (caller-supplied); **right** = an "Explore" mega-menu
trigger + a **GitHub** icon link + a **Discord** icon link. Sticky, matches the
existing accent tokens. `active` highlights the current section.

### Mega-menu (Lucide icons, two columns — like the reference screenshot)

- **Tools column** — a `<input type="search">` plus a results list rendered by
  `header.js`: fetch `/tools/_index.json` once, `filterTools` on input, render at
  most ~8 matches into a fixed-height, scrollable (windowed/virtualized)
  container; each row links `/tools/<slug>/` with title + description. The search
  itself is the browse-all mechanism (no separate index page) — search-first
  because the catalog is growing into the thousands, never a giant static list.
  On the static tool pages this also provides SEO-valuable internal links.
- **Resources column** — Lucide-iconed links with title + one-line subtitle:
  GitHub (repo), Discord (community), CLI (`cli/README.md`), SKILL.md
  (`SKILL.md`, for agents), About (info). Each opens in a new tab where external.

### Footer (`gizza-chrome::footer`)

Brand + the existing "free, private AI assistant…" blurb + link columns
(Tools / Resources). Renders on **tool pages** (replacing their current footer).
The **chat app gets no footer** — it's a viewport-filling app and all links are
reachable from the header's Explore dropdown.

### Icons

Inline SVG. **Lucide** for functional/category icons (search, chevron-down,
calculator, image, film/video, globe, terminal, bot/cpu, info). **GitHub and
Discord use their official brand marks** — Lucide has deprecated brand icons, so
depending on it for those is fragile. Icons live as small `Markup`-returning
helpers in `gizza-chrome` so both surfaces share exactly one copy.

### Chat-app integration (replace `sa-header`)

Replace the `sa-header` web component in `ui.rs` with `gizza-chrome::header`,
passing the animated-mascot markup as `brand`. Preserve the elements
`gizza-app.js` depends on so its logic keeps working:

- keep an `#open-settings` button (the ⋮ that JS relocates into the composer),
  now rendered as part of the shared header's actions area;
- keep the mascot markup (`.brand-mascot`, `#brand-still`, `#brand-video`,
  `.brand-eye*`) inside the `brand` block;
- the chat-only menu items (Info, WebGPU help, Clear conversation) stay in the
  existing `#composer-menu`; Discord/GitHub/About are additionally surfaced in
  the shared header/dropdown.

`gizza-app.js`'s `document.querySelector('sa-header h1')` lookup is updated to
target the new brand wordmark. This is the highest-risk change; it is covered by
`ui.rs`'s existing render tests plus a manual chat smoke check.

## File inventory

**New**
- `chrome/Cargo.toml`, `chrome/src/lib.rs` — `header()`, `footer()`, icon helpers (+ unit tests).
- `scripts/gen-seo.sh`, `scripts/gen-seo.test.sh`
- `site/header.css`, `site/header.js`, `site/tools-index.js`

**Edited**
- `Cargo.toml` (root) — add `gizza-chrome` path dep.
- `src/blocks/ui.rs` — SEO `<head>`; replace `sa-header` with shared header; load header assets; update tests.
- `site/gizza-app.js` — retarget the brand/`#open-settings` lookups.
- `site/tools-modal.js` — import `filterTools` from `tools-index.js`.
- `solobase.toml` — overlays + `extra_bypass_prefix` for `header.css`/`header.js`/`tools-index.js`.
- `tools/generator/Cargo.toml` — add `gizza-chrome` path dep.
- `tools/generator/src/main.rs` — drop sitemap/robots writes + `mod seo` + slug collection; copy header assets into each tool dir.
- `tools/generator/src/template.rs` — use shared header/footer; link header assets.
- `.github/workflows/deploy.yml` — build gizza CLI + run `gen-seo.sh`.
- `.github/workflows/test.yml` — run `gen-seo.test.sh` + chrome crate tests.
- `justfile` — `seo` recipe.

**Deleted**
- `tools/generator/src/seo.rs`

## Testing strategy

- **`gizza-chrome`:** `cargo test` — header contains brand passthrough +
  GitHub/Discord links + Explore trigger; footer contains the blurb + columns;
  icon helpers return non-empty SVG.
- **Generator:** existing `index`/`meta`/`template` tests; `template` tests
  updated for the shared header/footer markup.
- **`ui.rs`:** existing render tests + new assertions (SEO meta present; shared
  header present; `#open-settings` still present; mascot markup intact).
- **`gen-seo.test.sh`:** fixture-driven assertions listed under Project A.
- **JS:** `filterTools` stays unit-testable under `node:test` (the import move
  keeps it side-effect-free).
- **Manual smoke:** after deploy, verify `/sitemap.xml`, `/robots.txt`,
  `/llms.txt`, the header/dropdown on `/` and a tool page, and chat still boots
  (model picker, send, tools modal).

## Risks & mitigations

- **Replacing `sa-header`** (chat shell regression): keep the exact DOM hooks
  `gizza-app.js` needs; cover with render tests + manual smoke.
- **Deploy now builds the `gizza` CLI** (added cost/dependency): unavoidable
  consequence of "single source = the live registry"; `block.wasm` already
  exist at that point and the cargo cache is warm.
- **Tools search at thousands of entries:** windowed render + search-first;
  `_index.json` is already build-time static and fetched once.
- **Brand-icon fragility:** GitHub/Discord use official marks, not Lucide.

## Decisions (resolved during brainstorming)

- Sitemap lists only tools with a real page (no 404s), driven by `gizza list`.
- Build/ship Projects A and B together.
- Header on both surfaces; replace `sa-header` on the chat app.
- Dropdown = Resources links + a Tools **search** (windowed), not a static list.
- No footer on the chat app (links live in the header); full footer on tool pages.
