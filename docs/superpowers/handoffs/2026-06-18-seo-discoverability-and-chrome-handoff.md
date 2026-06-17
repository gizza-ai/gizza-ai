# Handoff — SEO/AI discoverability + shared chrome (gizza-ai)

**Date:** 2026-06-18
**Branch:** `feat/seo-discoverability-and-chrome`
**Worktree:** `.worktrees/seo-chrome` (gitignored; local only)
**Base:** `main` `461a701` (rebased onto it 2026-06-18, after #84/#85 merged)
**Spec:** `docs/superpowers/specs/2026-06-17-seo-discoverability-and-shared-chrome-design.md`

## Status: DESIGN COMPLETE — not yet implemented

Brainstorming is done, the spec is written, self-reviewed, and **reconciled with
merged `main`**. The next step is the **writing-plans** skill → implement (TDD) →
PR. No production/source code has been written yet; the branch holds only the
spec + this handoff.

Branch commits (on top of `461a701`):
- `9c00205` — design spec
- `5cc9a2a` — reconcile spec with merged #84/#85
- (this handoff commit)

## What this delivers (two coupled projects, ship together)

**Project A — discoverability**
- `scripts/gen-seo.sh` (new): generates `pkg/{sitemap.xml,robots.txt,llms.txt}`
  from `gizza list --json-out` (single source = live registry, no drift).
  - sitemap = apex + `/tools/<slug>/` only for tools that have a page
    (`blocks/<slug>/page/meta.toml` exists) → no 404s.
  - llms.txt = llmstxt.org format; page-tools link to the #85
    `/tools/<slug>/index.md` markdown twin; chat-only tools get name+description;
    closing pointer to `/tools/_index.json` + `gizza` CLI as the full catalog.
- Delete `tools/generator/src/seo.rs`; remove its sitemap/robots writes + `mod seo`
  + slug collection from `main.rs` (bash is now the only writer). **Keep** the #85
  `index.md` write + `mod markdown`.
- Fill the apex chat page `<head>` (`src/blocks/ui.rs render_chat()`): title, meta
  description, canonical, OG, Twitter card, JSON-LD Organization+WebSite (it is
  currently SEO-bare — biggest single win).
- `solobase.toml`: add `/sitemap.xml` + `/robots.txt` + `/llms.txt` to
  `extra_bypass_prefix` (#85 deferred this to us).
- CI: `deploy.yml` builds the `gizza` CLI (`cargo install --path gizza-ai/cli`,
  no `--locked`) then runs `scripts/gen-seo.sh`; `test.yml` runs `gen-seo.test.sh`
  + the new chrome crate tests. `justfile` gets a `seo` recipe.

**Project B — shared chrome**
- New leaf crate `chrome/` (package `gizza-chrome`, `maud` only): `header(brand, active)`
  + `footer()` + inline-SVG icon helpers. Consumed by BOTH `src/blocks/ui.rs`
  (wasm chat) and `tools/generator/src/template.rs` (native) via path deps — one
  copy of the markup, no drift.
- Header on both surfaces: brand (caller-supplied: mascot on chat, static logo on
  tool pages) + GitHub + Discord icons + an "Explore" Lucide mega-menu.
- Mega-menu = 2 columns: **Tools** = a *search* over `/tools/_index.json`
  (windowed/virtualized, ~8 shown — the catalog is heading into the thousands,
  see the `gizza-search-tools` backlog at 1000) + **Resources** = GitHub, Discord,
  CLI, SKILL.md, About.
- Footer on tool pages (replaces the current one); **no footer on the chat app**
  (links live in the header dropdown).
- Icons: **Lucide** for functional/category icons; **GitHub/Discord = official
  brand marks** (Lucide deprecated brand icons).
- Shared client assets (new): `site/header.css`, `site/header.js`,
  `site/tools-index.js` (the pure `filterTools`, imported by both `header.js` and
  the existing `tools-modal.js`). Delivered to chat via `solobase.toml` overlays +
  bypass; copied into each tool dir by the generator.
- Replace `sa-header` on the chat app with `gizza-chrome::header` — **highest-risk
  change**. Preserve the DOM hooks `gizza-app.js` needs: keep `#open-settings`
  button + the mascot markup (`.brand-mascot`/`#brand-still`/`#brand-video`/`.brand-eye*`);
  retarget `gizza-app.js`'s `querySelector('sa-header h1')` to the new wordmark.

Full file inventory + testing strategy are in the spec.

## Decisions already locked (from brainstorming)

- Sitemap lists only tools with a real page (no 404s), driven by `gizza list`.
- Build & ship Projects A and B **together** (one combined effort).
- Header on **both** surfaces; **replace** `sa-header` on the chat app.
- Dropdown = Resources links + a Tools **search** (windowed), not a static list.
- No footer on the chat app; full footer on tool pages.
- GitHub/Discord brand marks; Lucide for everything else; inline SVG.

## Coordination with merged #85 (md-tool-pages) — important

#85 shipped per-tool `index.md` twins + `<link rel=alternate>` and **deferred**
`/llms.txt` + sitemap + robots + the SW-bypass + `seo.rs` deletion to THIS effort
(commit `e300b09`). Do **not** touch `tools/generator/src/markdown.rs` behavior;
we only link into its output. Only shared file is generator `main.rs` (already
rebased clean).

## How to resume (cold-start checklist)

1. **Shared-tree hazard:** `ps aux | grep [c]laude` and `git -C <gizza-ai> worktree list`.
   Multiple sessions share the one working tree; do branch ops in the worktree, not
   the shared `main` checkout. See memory `workspace-concurrent-session-git-hazard`.
2. `cd /home/joris/Programs/suppers-ai/workspace/gizza-ai/.worktrees/seo-chrome`
   (if the worktree was cleaned: `git worktree add .worktrees/seo-chrome feat/seo-discoverability-and-chrome`
   from the repo root; the branch ref survives even if the worktree dir is gone).
3. Re-read the spec (link above). Confirm with the user it still reflects intent
   (they had not given final "go to writing-plans" approval — they paused here).
4. Invoke the **writing-plans** skill to turn the spec into a step-by-step plan,
   then implement TDD (chrome crate + gen-seo.sh test-first), open a PR.
5. Baseline before coding: `cargo test --manifest-path tools/generator/Cargo.toml`
   (fast, the CI gate for the generator). A full `cargo build` pulls heavy
   wafer-run/solobase git+path deps.

## Not yet done / open

- User has NOT approved moving to writing-plans (paused to hand off).
- Branch is **local only** — not pushed. Push if you want remote durability
  (`git push -u origin feat/seo-discoverability-and-chrome`); commits are already
  safe in the local repo's branch ref regardless of the worktree.
- No implementation, no PR yet.
