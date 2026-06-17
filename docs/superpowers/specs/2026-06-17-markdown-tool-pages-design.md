# Markdown tool-page twins (per-tool `index.md`)

**Date:** 2026-06-17
**Status:** Implemented + unit-tested (generator suite green) and verified by a
real `gizza-tool-pages` run — `index.md` produced for all 5 page-tools
(field/file/live variants correct) and each `index.html` links its twin via
`<link rel="alternate" type="text/markdown">`. Branch `feat/md-tool-pages`.

## Problem

gizza's per-tool standalone pages (`gizza.ai/tools/<slug>/index.html`) are
HTML+JS — noisy for an LLM that fetches the page (via WebFetch/crawl) to learn
what a tool does and how to call it. Every tool already authors its prose as
`blocks/<slug>/page/content.md`, which the generator renders into the HTML; the
clean markdown is the *source*, but it isn't served.

## Goal

Emit, inside the existing `tools/generator` pass, a clean **`index.md`** twin next
to every tool's `index.html`, single-sourced from the same `meta.toml` +
`content.md` that drive the HTML — so web-browsing LLMs can read a tool without
parsing HTML, and so adding a tool yields its `.md` automatically with zero drift
and no new build step.

## Relationship to the SEO/discoverability effort (important)

A **separate, concurrent effort** (`feat/seo-discoverability-and-chrome`,
design `2026-06-17-seo-discoverability-and-shared-chrome-design.md`) owns the
root **`/llms.txt`**, `sitemap.xml`, and `robots.txt`, generated from `gizza
list` (the live registry — all ~14 tools, not just the 5 with pages) via
`scripts/gen-seo.sh`. That is the better home for the *index* surface (live
registry = single source of truth; covers chat-only tools too).

**This spec therefore does NOT produce `/llms.txt`.** The two efforts are
complementary: that effort's `llms.txt` is the index over all tools; this
effort's per-tool `index.md` is the per-page detail twin. The SEO effort's
`llms.txt` entries for the 5 page-tools can link to `/tools/<slug>/index.md`
(coordination note for that effort; no code dependency here).

## Key decision: schema source = `meta.toml` (not `block.info()`)

`meta.toml` already declares the page's I/O: each `[[input]]`
(`name`/`source`/`label`/`accept`) plus `output_label`/`format`/`runtime`. The
generator is a pure static pass over `meta.toml` + `content.md`, and the HTML page
renders from the same `meta.toml`. Using it for the `.md` keeps the twin
**consistent with the HTML page** and avoids invoking the runtime/CLI mid-build.
The canonical *runtime* schema stays `gizza describe` / `block.info()`; the `.md`
documents the page surface and points agents at the CLI for invocation.

## Components (all inside `tools/generator`)

### New: `tools/generator/src/markdown.rs`

`tool_markdown(meta: &ToolMeta, content_md: &str) -> String` → the per-tool
`index.md`:

```markdown
# {h1}

{description}

## Run it

- **CLI:** `gizza tool {slug} "{first field input's placeholder}"`
- **Web:** https://gizza.ai/tools/{slug}/

## Inputs

- `{name}` — {label} _( {source}{; accept: {accept}} )_
… one per manual (field/file) [[input]]

## Output

- {output_label} ({format})

---

{content.md, verbatim}
```

Rules:
- The CLI line uses the first `source = "field"` input's `placeholder` as the
  example arg; a file-only tool renders `gizza tool {slug} <path>`; an
  auto-only tool (e.g. clock, whose only input is `source = "clock"`) renders
  `gizza tool {slug}` and an `## Inputs` note that it takes no manual arguments.
- The `accept` clause is omitted when empty.

### Modify: `tools/generator/src/main.rs`

After writing `index.html` per tool, also write
`out.join("index.md")` = `markdown::tool_markdown(m, &content_md)`. Register
`mod markdown;`. (No `llms.txt` write — that's the SEO effort's job.)

### Modify: `tools/generator/src/template.rs`

Add to each page `<head>`:
`<link rel="alternate" type="text/markdown" href="index.md">` (relative → resolves
to `/tools/<slug>/index.md`).

### No `solobase.toml` change

`index.md` lives under `/tools/`, already covered by the existing `/tools/` SW
fetch-bypass and served statically. (`/llms.txt` bypass, if any, belongs to the
SEO effort.)

## Single source / no drift

`meta.toml` (metadata + I/O schema) and `content.md` (prose) are the only
sources. `index.html`, `index.md`, and `_index.json` all derive from them.
Adding a tool automatically produces its `.md` — no per-tool maintenance.

## Testing

- **`tools/generator` unit tests** (mirror the existing `index.rs`/`meta.rs`
  style; run by `cargo test --manifest-path tools/generator/Cargo.toml`, a CI
  gate):
  - `tool_markdown` for a field tool (calculator) contains the `# {h1}` header,
    the `gizza tool calculator "2 + 2 * 3"` CLI line, the
    `https://gizza.ai/tools/calculator/` URL, the `expr` input line, the
    `Result (number)` output line, and the appended prose.
  - `tool_markdown` for a file tool renders `gizza tool <slug> <path>` and an
    `accept` clause.
  - `tool_markdown` for an auto-only (`live`/clock) tool renders the no-manual-
    arguments note and `gizza tool <slug>` (no quoted arg).
  - `template.rs` head test asserts the `<link rel="alternate" type="text/markdown" href="index.md">`.
- **Full generation** (deploy pipeline / manual, once per-tool wasms exist):
  running `gizza-tool-pages` writes `pkg/tools/<slug>/index.md` for every tool.

## Scope boundaries

- Separate track from the chat-ffmpeg page-side bridge (`feat/chat-ffmpeg-page-side-bridge`, parked).
- Does **not** touch `/llms.txt`, `sitemap.xml`, `robots.txt`, or `seo.rs` — those
  belong to `feat/seo-discoverability-and-chrome`. This feature only adds the
  per-tool `index.md` + its discovery `<link>`, avoiding any collision with that
  effort's generator changes.
