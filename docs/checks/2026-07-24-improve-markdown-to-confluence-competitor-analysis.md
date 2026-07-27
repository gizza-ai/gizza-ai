# markdown-to-confluence — competitor analysis (2026-07-24)

Snapshot for the build/improve pass. All observations are **paraphrased** — no
competitor copy, branding, or trademarks reproduced. Analysis is for
feature/UX ideas only.

## Competitors scanned (top 3 reachable real tools)

### A — code-utility "markdown to confluence" (toolsbox-style)
- **Output:** Confluence **storage format** (XHTML-based markup with custom
  `<ac:…>` macros).
- **Constructs:** headings h1–h6, bold/italic, links, images, ordered +
  unordered + nested lists, code blocks, tables, blockquotes.
- **Macros:** fenced code → structured `code` macro; blockquotes prefixed
  `Note:` / `Warning:` / `Info:` → the matching panel macros.
- **UX:** paste textarea, Convert button, Reset, copy-to-clipboard, real-time.

### B — md-to converter
- **Output:** Confluence **wiki markup** (targets Data Center / Server / legacy
  markup-insert flow; notes the Cloud editor won't paste wiki markup directly).
- **Constructs:** headings, code blocks with language tag, tables (structure
  preserved), internal + external links, images, bold/italic.
- **UX:** paste/type, Load Example, Browse File + drag-drop upload, one-click
  copy, undo/redo, save shortcut, live preview.

### C — markdownme converter
- **Output:** **both** wiki markup (default) and storage format (XHTML) via a
  toggle — the storage option is framed for REST-API compatibility.
- **Constructs:** headings all levels, bold/italic/strikethrough, fenced code
  with language tags, pipe tables, links, images, blockquotes.
- **Special blocks:** blockquotes prefixed `Note:` / `Warning:` / `Info:` /
  `Tip:` auto-convert to the matching Confluence panel macros.
- **UX:** paste input, optional starter templates (runbook / spec / incident /
  meeting notes), output-format toggle, copy button, live preview.

## Table-stakes distilled

| Capability | Verdict | Where |
| --- | --- | --- |
| Storage format (XHTML) output | in-model | `format=storage` |
| Wiki markup output | in-model | `format=wiki` |
| Output-format toggle | in-model | `format` enum (select) |
| Headings h1–h6 (+ demote/offset) | in-model | offset param |
| Bold / italic / strikethrough / inline code | in-model | inline emitter |
| Links + images | in-model | inline emitter |
| Ordered / unordered / nested / task lists | in-model | list emitter |
| Fenced code → code macro (with language) | in-model | code emitter |
| Pipe tables | in-model | table emitter |
| Blockquotes | in-model | quote emitter |
| Note/Warning/Info/Tip panel macros from `> Prefix:` | in-model | `panel_blockquotes` bool |
| Thematic breaks | in-model | `<hr/>` / `----` |
| All-local, no upload, no signup | already ours | wasm |

## Decisions

- **`format` enum** `storage` (default) \| `wiki` — renders a `<select>`.
  Default `storage` because it is the lossless canonical format the Cloud REST
  API consumes and the only one that carries macros/panels reliably; wiki
  markup is offered for the legacy Data Center / Server "Insert markup" flow.
- **`panel_blockquotes` bool** (default `true`) — a blockquote whose first line
  starts `Note:` / `Warning:` / `Info:` / `Tip:` (case-insensitive) becomes the
  matching Confluence panel macro (`info`/`note`/`warning`/`tip`), stripping the
  prefix. Turn off for literal `<blockquote>` / `{quote}`.
- **`heading_offset` int 0–5** — demote every heading N levels (Confluence caps
  at h6). Mirrors our markdown-to-latex tool's family-consistent knob.
- **Original copy only** — page copy, examples, and FAQ authored fresh.

## Out-of-model / considered, not built

- **Starter templates** (competitor B/C) — could ship as `[[example]]` preset
  chips instead of a template picker; added runbook/table/panel example chips.
- **Direct upload to a Confluence space / REST push** — needs an account,
  API token, and a server round-trip; out of the browser-local/wasm model.
- **File upload + drag-drop of a `.md` file** — the page's paste textarea covers
  the same input; a file picker is a site-repo chrome concern, not the block.
- **Live rendered Confluence preview** — would require embedding Confluence's
  renderer; we show the exact markup a user pastes instead.
