# markdown-strip — competitor analysis (2026-07-31)

Tool: strip all Markdown formatting (headings, emphasis, links, code fences,
tables, blockquotes, lists, images, rules) to produce clean plain text. Pure,
browser-local, no account. All findings below are **paraphrased** — no competitor
copy, branding, or trademarks reproduced.

## Search

`WebSearch`: "strip markdown formatting to plain text online tool" and
"remove markdown convert to plain text tool". The space is crowded with near-
identical single-textarea converters. Three reachable ones carried enough detail
to profile; several others (thetexttool, markdownpaste, removemarkdown,
picotoolkit) were single-line landing pages or returned 403 and yielded no
feature detail.

## Competitor profiles (paraphrased)

### 1. MarkdownTools — remove-markdown (markdowntools.io)
- **Options:** a single **"keep list markers"** toggle (preserve `-` bullets and
  `1.` numbering when on; strip them when off).
- **Behavior:** links keep their visible text and drop the `[text](url)` markup;
  images collapse to their alt text; fenced code content is preserved verbatim
  with only the backtick fences removed; tables lose their pipes leaving bare
  cell text spread across lines; headings lose their `#`; bold/italic/
  strikethrough markers are removed; blockquote `>` markers removed; horizontal
  rules removed.
- **Worked example:** a quarterly-summary doc with bold, links, strikethrough,
  code, and a table converted to readable plain text.
- **Limits:** none stated; runs fully in-browser, nothing uploaded.

### 2. RemoveMD (removemd.org)
- **Options:** no user-facing toggles — a paste box, Convert, Copy Result, Clear.
- **Behavior:** removes headings, bold, italic, links, lists, code blocks, inline
  code, images, and blockquotes. Additionally converts LaTeX math to Unicode
  symbols (α, β, π, ∑, ∫).
- **Positioning:** markets cleaning of AI chat output (ChatGPT/Claude).
- **Limits:** none stated; in-browser, no data sent.

### 3. wtools.io Strip Markdown (wtools.io/strip-markdown)
- **Options:** none surfaced; removes Markdown syntax and leaves only the plain
  text content.
- **Behavior:** general strip-to-plain-text of all common Markdown constructs.

## Table-stakes params + decisions (in-model / out-of-model)

| capability | seen at | decision | tag |
| --- | --- | --- | --- |
| Strip headings/emphasis/strikethrough | all | core behavior | in-model |
| Links → keep visible text, drop URL | markdowntools, all | default `links=text` | in-model |
| Links → keep the URL / keep both | (implied gap) | add `links` enum: `text`\|`url`\|`both` | in-model |
| Images → alt text vs drop | markdowntools (alt) | `images` enum: `alt`\|`drop` (default `alt`) | in-model |
| Keep list markers toggle | markdowntools | `keep_list_markers` boolean (default false) | in-model |
| Collapse multiple blank lines | (readability) | `collapse_blank_lines` boolean (default true) | in-model |
| Preserve fenced code content | markdowntools, all | always keep code content (no toggle) | in-model (default) |
| Tables → bare cell text | markdowntools | join cells with spaces, one row per line | in-model |
| Blockquotes → strip `>` | all | core behavior | in-model |
| Horizontal rules removed | markdowntools | core behavior | in-model |
| LaTeX math → Unicode symbols | removemd | niche; a full LaTeX→Unicode map is a separate tool's job | considered, rejected |
| File upload / batch export | some | browser-local single paste covers it; page has Copy + Download | out-of-model (server batch) |

## UX control patterns adopted

- Multiline paste textarea for the Markdown input (worked-example placeholder).
- `links` / `images` render as `<select>` (enum → manifest → generator control).
- `keep_list_markers` / `collapse_blank_lines` render as checkboxes.
- `[[example]]` preset chips demonstrating link/list/table handling.
- Copy result + Download (text format) come free from the generator; FAQ as
  `<details>` accordions; limits stated on the page.

## Engine decision

Parse with `pulldown-cmark` 0.12 (already proven wasm-safe in this repo — used by
`markdown-render`) and walk the event stream to emit plain text. This is more
robust than line-regex stripping for nested emphasis, reference links, and
tables, and reuses a vetted dependency.
