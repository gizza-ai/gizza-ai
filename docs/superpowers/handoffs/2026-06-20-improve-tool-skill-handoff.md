# Handoff — `/improve-tool` skill (gizza-ai)

**Date:** 2026-06-20
**Status:** NOT STARTED — design first (brainstorm → spec → plan), then build. Nothing on disk yet.
**Owner of the idea:** user (raised 2026-06-19 alongside the shared-code idea).

## The idea (verbatim intent)

A `/improve-tool` skill that, for an existing gizza tool, **analyzes/searches the top ~5
competitors, takes snapshots of their version of the tool, and improves our tool** to match
or beat them. Sibling to the existing `/new-tool` skill (which *creates* tools).

## Why now (what just shipped makes this easy)

The **shared-tool-abstraction program (Plans 1–5) shipped 2026-06-19** — every one of the 25
tools is now a uniform shape: a single `core::`/`src/lib.rs` `descriptor()` that single-sources
the chat schema (+ CLI + page + URL query params), with logic in a pure `core` crate and the
wrapper collapsed into `block-utils` helpers (`run_skill` / `resolve_source` / `dispatch_ffmpeg`
/ `build_media_envelope` / `ArgvPlan`). See memory `[[gizza-shared-tool-abstraction-2026-06-19]]`
and `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md`.

**Consequence:** improving a tool is now mostly "edit `descriptor()`'s params + `core`'s logic"
— mechanical and safe. The retrofit's per-tool drift-guard test was a *migration* guard (derived
== authored); `/improve-tool` **intentionally changes** the schema, so it must *regenerate* the
expected schema, not assert against the old one.

## Sketched flow (refine in brainstorming — do NOT treat as final)

1. **Input:** an existing tool `slug` (+ optional focus, e.g. "more formats").
2. **Research:** find the top ~5 competitor tools for that function (web search).
3. **Snapshot:** capture each competitor's tool — features, params/options, defaults, UX, output
   format/quality.
4. **Analyze/diff:** compare our tool (its `descriptor()` params + `core` behavior) against the
   competitor set → a gap list (missing params, weak defaults, missing formats, UX, output quality).
5. **Improve:** edit our tool (descriptor params, core logic, page copy) to close the highest-value
   gaps — through the SAME build/test/PR loop `/new-tool` uses. Update/replace the drift-guard to the
   new intended schema; keep all behavior tests green.
6. **Ship:** review-only PR with the changes **plus a competitor-analysis summary** (what was found,
   what changed, why); do NOT merge.

## Open questions to resolve in brainstorming (the real design work)

- **Scope per run:** improve ONE named tool (recommended start) vs a sweep? How much change per run
  (avoid over-editing a working tool / scope creep)?
- **Competitor research:** how many, how chosen, what exactly to extract (features / params / output
  samples / pricing / UX)? Trust threshold for "this is a real competitor"?
- **Snapshot method:** `firecrawl-search` + `firecrawl-extract` (structured competitor features) vs
  `firecrawl-scrape` (markdown) vs Playwright MCP (visual/interactive)? Likely a mix.
- **"Improvement" decision rule:** what gets applied autonomously vs flagged for human review? Guard
  against degrading a tool or chasing competitor features that don't fit the browser-local model.
- **Drift-guard handling:** since the schema changes on purpose, the skill regenerates the expected
  schema + documents the before→after diff in the PR (LLM-facing schema changes are the whole point
  here, unlike the retrofit).
- **Verification:** reuse `/new-tool`'s gates (`cargo test --workspace`, `wafer build`, `wasm-pack`
  for page tools, `gizza tool <slug>` CLI smoke). Plus a "did we actually improve it" check.
- **Output artifact:** the competitor-analysis summary — where (PR body? a `docs/` note?).

## Tools available

- **Firecrawl skills:** `firecrawl-search`, `firecrawl-scrape`, `firecrawl-extract` (structured JSON
  per a schema — good for competitor feature tables), `firecrawl-map`, `firecrawl-agent`.
- **Playwright MCP** (browser snapshots/screenshots of competitor tools).
- **The tool toolchain:** the `/new-tool` build/test/PR loop applies (the tool already exists, so no
  scaffold) — `scripts/scaffold-tool.sh` is only for new tools.
- **The descriptor abstraction** (Plans 1–5) — how to edit any tool's schema/logic safely.

## Where it will live

`gizza-ai/.claude/skills/improve-tool/` (`SKILL.md` + `reference.md`), sibling to `new-tool/`.

## Pointers / references

- **Structural template:** `gizza-ai/.claude/skills/new-tool/SKILL.md` + `reference.md` (an
  autonomous, name→shipped-PR tool skill — mirror its shape, honesty gate, build/test steps; they
  were updated 2026-06-19 to the descriptor shape).
- **The tool shape to edit:** spec `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md`;
  exemplars `blocks/url-encode` (pure), `blocks/image-resize` (media page), `blocks/web-fetch` (no-page).
- **Toolchain gotchas:** `/workspace/docs/checks/2026-06-18-gizza-new-tool-build-notes.md` (NOTE: written
  pre-descriptor; the new-tool `SKILL.md`/`reference.md` are the current source of truth).
- **Memory:** `[[gizza-shared-tool-abstraction-2026-06-19]]` (what shipped), `[[gizza-new-tool-skill-2026-06-17]]`.

## First step tomorrow

Invoke the **brainstorming** skill on `/improve-tool` (it's creative work — resolve the open
questions above into a design before writing any skill files). Then spec → plan → build, exactly
like `/new-tool` and the abstraction were done.
