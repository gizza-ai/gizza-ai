# Markdown-for-LLMs tool pages

**Date:** 2026-06-17
**Status:** Design approved; ready for implementation plan.

## Problem

gizza's per-tool standalone pages (`gizza.ai/tools/<slug>/index.html`) are
HTML+JS — noisy for an LLM that fetches the page via WebFetch/crawl to learn what
a tool does and how to call it. gizza already serves agent-facing surfaces (the
`gizza` CLI, `SKILL.md`, `/tools/_index.json`, `block.info()` schemas), but none
of those help an agent that simply lands on a tool page on the web. The emerging
convention for this is a markdown twin of each page plus a root `/llms.txt`
index. There is no markdown-for-agents surface today.

## Goal

Emit, inside the existing `tools/generator` pass:

1. a clean **`index.md`** twin next to every tool's `index.html`, and
2. a root **`/llms.txt`** index linking them,

both single-sourced from the `meta.toml` + `content.md` the generator already
reads — so adding a tool yields its `.md` + `llms.txt` line automatically, with
zero drift and no new build step.

## Key decisions

### Schema source = `meta.toml` (not `block.info()`)

`meta.toml` already declares the page's I/O: each `[[input]]`
(`name`/`source`/`label`/`accept`) plus `output_label`/`format`/`runtime`. The
generator is a pure static pass over `meta.toml` + `content.md`, and the HTML
page renders from the same `meta.toml`. Using it for the `.md` keeps the twin
**consistent with the HTML page** and avoids invoking the runtime/CLI mid-build.
The canonical *runtime* schema stays `gizza describe` / `block.info()`; the `.md`
documents the page surface and points agents at the CLI for invocation.
(Rejected: shelling out to `gizza describe` during generation — more coupling,
risks page/HTML drift.)

### `/llms.txt` is an index, not a full dump

Per the llms.txt convention, `/llms.txt` is a title + summary + a link list to
the per-tool `.md` files — **not** a concatenation of every tool's content. With
gizza's tool backlog a full-dump file would balloon; the index + per-tool `.md`
scales and matches the convention's index/`llms-full.txt` split. No
`llms-full.txt` in this scope.

### Serving (verified)

The runtime SW (`sw.js`) intercepts **all** same-origin requests except its
bypass list. Crawlers/agents fetch without a registered SW, so static files at
the origin (Cloudflare Pages serving `pkg/`) are returned directly — `/llms.txt`
works for web-fetching LLMs with **no** bypass needed. To also work in a real
browser tab that already has the gizza SW registered, add `/llms.txt` to
`extra_bypass_prefix`. Per-tool `index.md` lives under `/tools/`, already covered
by the existing `/tools/` bypass.

> Observed latent gap (out of scope, noted): `robots.txt` and `sitemap.xml` are
> likewise **not** bypassed, so they 404 in a SW-registered browser tab (they
> work for crawlers only). The plan may optionally add `/robots.txt` +
> `/sitemap.xml` to the bypass in the same one-line change for consistency.

## Components (all inside `tools/generator`)

### New: `tools/generator/src/markdown.rs`

- `tool_markdown(meta: &ToolMeta, content_md: &str) -> String` → the per-tool
  `index.md`:

  ```markdown
  # {h1}

  {description}

  ## Run it

  - **CLI:** `gizza tool {slug} "{first field input's placeholder}"`
  - **Web:** https://gizza.ai/tools/{slug}/

  ## Inputs

  - `{name}` — {label} _( {source}{; accept: {accept}} )_
  … one per [[input]]

  ## Output

  - {output_label} ({format})

  ---

  {content.md, verbatim}
  ```

  Rules:
  - The CLI line uses the first `source = "field"` input's `placeholder` as the
    example arg; for tools whose only input is a `file`, render
    `gizza tool {slug} <path>` instead (with a note the input is a file).
  - The `accept` clause is omitted when empty.
  - `live` tools (clock-style, no field inputs) render an `## Inputs` note that
    the tool takes no arguments.

- `llms_txt(metas: &[ToolMeta]) -> String` → the root `/llms.txt`:

  ```markdown
  # gizza.ai — browser-native tools

  > Free, single-purpose tools that run entirely in your browser. Many also run
  > headlessly via `gizza tool <name>` (see the CLI + SKILL.md in the repo).

  ## Tools

  - [{title}](https://gizza.ai/tools/{slug}/index.md): {description}
  … one per tool, in the generator's existing order
  ```

  Links are **absolute** (`https://gizza.ai/...`) so an `llms.txt` fetched
  standalone via WebFetch resolves them correctly.

### Modify: `tools/generator/src/main.rs`

- After writing `index.html` per tool, also write
  `out.join("index.md")` = `markdown::tool_markdown(&m, &content_md)`.
- After the existing `_index.json` / `sitemap.xml` / `robots.txt` writes, write
  `pkg.join("llms.txt")` = `markdown::llms_txt(&metas_only)`.
- Register `mod markdown;`.

### Modify: `tools/generator/src/template.rs`

- Add to each page `<head>`:
  `<link rel="alternate" type="text/markdown" href="index.md">` (relative, so it
  resolves to `/tools/<slug>/index.md`).

### Modify: `gizza-ai/solobase.toml`

- Append `"/llms.txt"` to `[assets].extra_bypass_prefix`. (Optionally
  `"/robots.txt"`, `"/sitemap.xml"` per the noted gap.)

## Single source / no drift

`meta.toml` (metadata + I/O schema) and `content.md` (prose) are the only
sources. `index.html`, `index.md`, `_index.json`, `sitemap.xml`, and `llms.txt`
all derive from them. Adding a tool automatically produces its `.md` and
`llms.txt` entry — no per-tool maintenance.

## Testing

- **`tools/generator` unit tests** (mirror the existing `index.rs`/`meta.rs`
  test style):
  - `tool_markdown` for a field tool (calculator) contains: the `# {h1}` header,
    the `gizza tool calculator "2 + 2 * 3"` CLI line, the
    `https://gizza.ai/tools/calculator/` URL, the `expr` input line, the
    `Result (number)` output line, and the appended prose.
  - `tool_markdown` for a file tool (e.g. image-grayscale) renders the
    `gizza tool <slug> <path>` form and an `accept` clause.
  - `tool_markdown` for a `live` tool (clock) renders the no-arguments inputs
    note.
  - `llms_txt` over multiple metas lists every tool as
    `- [title](/tools/<slug>/index.md): description`, in order.
- **Generator integration** (the generator's existing end-to-end test, if any):
  assert `index.md` is written for each tool and `llms.txt` at pkg root.
- **`sw-bypass.test.js`**: assert `/llms.txt` is bypassed in the built
  `pkg/sw.js`.

## Scope boundary

Separate track from the chat-ffmpeg page-side bridge work (that plan is parked,
awaiting an execution-approach choice). This feature touches only
`tools/generator` + one `solobase.toml` line; no runtime, block, or SW-template
changes.
