# render-template — competitor analysis (2026-06-21)

Tool: **render-template** — renders a Handlebars/Mustache template against supplied
JSON data and returns the output text. Pure-compute (Rust `handlebars` 6, runs in
the chat WASM sandbox, the CLI, and the browser page). Three surfaces verified:
chat block (`wafer build` OK), CLI (`gizza tool render-template …`), page Playwright.

## Surfaces verified

- **Chat / LLM API:** `wafer build` validates `target/block.wasm` (796 KiB) — `handlebars`
  instantiates in `wasm32-wasip1`. Schema drift-guard test passes.
- **CLI:** basic `{{var}}`, `{{#each}}` loops, `{{user.name}}` nested paths,
  `{{#if}}/{{else}}` conditionals, `engine=mustache`, and `strict=true` error path
  all confirmed.
- **Page:** 3 Playwright tests pass (variable substitution, each+if, strict-mode error).

## Competitors surveyed

| Competitor | What it offers | Notes |
|---|---|---|
| hbsplayground.xyz | Handlebars compile + live preview, JSON editor, syntax highlight, .hbs/.json upload | Handlebars only; in-browser |
| tryhandlebarsjs.com | Official "try Handlebars in your browser" playground | Handlebars only |
| handlebarsjs.com docs | Reference + inline examples | Docs, not a renderer |
| micromustache (GitHub) | Tiny `{{mustache}}` subset for JS | Library, not a hosted tool |
| HTMX client-side-templates | Renders mustache/handlebars `<script>` templates client-side | Framework extension, not a standalone renderer |

Sources:
- [Handlebars Template Renderer — hbsplayground.xyz](https://hbsplayground.xyz/)
- [handlebarsjs.com](https://handlebarsjs.com/)
- [Mustache (template system) — Wikipedia](https://en.wikipedia.org/wiki/Mustache_(template_system))
- [handlebars — Rust docs](https://docs.rs/handlebars/latest/handlebars/)
- [micromustache — GitHub](https://github.com/alexewerlof/micromustache)

## Gap analysis (fit-to-model)

Closed / already covered:

- **Variable substitution, nested paths, `{{#each}}`, `{{#if}}/{{else}}`** — the core
  features of every competitor playground. All supported via the `handlebars` engine
  (a superset of Mustache), exercised by unit + CLI + Playwright tests.
- **Mustache compatibility** — exposed as an `engine` selector (`handlebars` | `mustache`);
  both render on the Handlebars engine, matching the documented Mustache↔Handlebars swap.
- **Missing-variable behaviour** — added a `strict` toggle: lenient (default, renders empty,
  matching Mustache/Handlebars defaults) vs strict (error), which the playgrounds don't
  expose as a first-class control.
- **No HTML-escaping by default** — `register_escape_fn(no_escape)` so the tool is usable
  for emails / config / code generation, not just HTML output. Competitors HTML-escape by
  default, which surprises non-HTML users.
- **Privacy / no upload** — like the in-browser playgrounds, runs entirely client-side
  (page) or in the sandbox (chat); nothing is sent to a server.

Deliberately out of scope (out-of-model or not a fit):

- **Custom helpers / partials registration** — competitors that let you register arbitrary
  JS helpers can't be matched in a pure, sandboxed, deterministic tool (arbitrary code
  execution); built-in helpers (`each`, `if`, `unless`, `with`, `lookup`) are supported.
- **File upload of `.hbs`/`.json`** — the page is a paste-in text tool; file-input wiring
  for two text files isn't part of the page model. Paste covers the same use case.
- **Syntax highlighting in the editor** — a front-end editor feature, orthogonal to the
  compute tool; the page uses plain multiline fields.

## Copy / UX

- Page title, h1, hero, tags, and SEO copy written fresh (no competitor copy/branding copied).
- Multiline fields for template + data (preserve pasted newlines); `engine` renders as a
  `<select>`, `strict` as a checkbox (descriptor-driven). Example template/data shown in
  placeholders and `content.md`.
