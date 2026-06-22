# html-to-markdown — competitor analysis (2026-06-20)

Tenth `/create-next-tool` backlog pick. Pure text tool (Input::None, htmd) — full
3 surfaces (chat / CLI / page + query-param deep-link). Research via `WebSearch`,
paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| tomarkdown.org | GFM: tables, code blocks, task lists; in-browser, no upload | capabilities |
| markdownlang / ToolsLab | real-time; GFM tables, fenced code, strikethrough, task lists | capabilities |
| htmltomarkdown.io | GFM toggle: pipe tables, task lists [ ]/[x], strikethrough; in-browser | capabilities |
| htmlmarkdown.com | strikethrough, tables, task-list items | capabilities |

## Gap diff vs our tool
Our tool (htmd): preserves **headings, links, images, ordered/unordered lists
(incl. nesting), inline + fenced code, GFM pipe tables, blockquotes, bold/italic,
HRs** — verified the table output is a proper `| A | B |` GFM table.

**In-model gaps considered, deferred (fit the model; minor):**
- **Strikethrough** (`<del>`/`<s>` → `~~text~~`) — htmd 0.5 currently drops the
  marker (renders plain text). Would need a custom element handler or a post-pass.
- **Task lists** (`<input type=checkbox>` → `- [ ]`/`- [x]`) — not emitted by htmd.
- A bullet-style option (htmd uses `*`; some users prefer `-`).

**Out-of-model:** URL/Word/PDF → Markdown (those are separate input types — e.g. a
future fetch-url-to-markdown built on web-fetch, or a docx tool); real-time
preview pane (the page recomputes on input, which is effectively live).

## Tested
unit (5: headings+bold, links, unordered list bullets, fenced code block, empty
input error) + drift-guard · wafer fixtures (1) · `wafer build` validates the
block · wasm-pack web · generator · CLI (verified clean Markdown incl. a GFM pipe
table) · Playwright page (conversion + query-param deep-link, 2 tests).

> Original work only — no competitor copy, branding, or trademarks copied.
