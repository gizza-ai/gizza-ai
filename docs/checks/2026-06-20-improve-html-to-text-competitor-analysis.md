# html-to-text — competitor analysis (2026-06-20)

Eleventh `/create-next-tool` backlog pick. Pure text tool (Input::None,
nanohtml2text) — full 3 surfaces. Distinct from html-to-markdown: this outputs
PLAIN text (no Markdown markers). Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| various "strip HTML tags" tools | remove tags, decode entities, keep line breaks | capabilities |
| browserling / textfixer | paste HTML → plain text, in-browser | UX |
| html2text libraries (python/php) | configurable wrapping, link handling, ignore-images | capabilities |

## Gap diff vs our tool
Our tool: removes all tags/attributes, decodes HTML entities, keeps paragraph
breaks + list items on their own lines, normalizes CRLF→LF, collapses excess
blank lines. Output is true plain text (no `#`/`**` markers — verified; that's
the deliberate difference from the sibling html-to-markdown tool).

**In-model gaps considered, deferred (fit the model; minor):**
- **Line-wrap width** — optionally hard-wrap to N columns (some libraries do).
  We deliberately don't wrap (preserves the source's own breaks); a `width` opt
  is an easy future add. (Tried html2text for this but it injects Markdown-ish
  markers, so nanohtml2text was chosen for clean output instead.)
- **Link handling** (append URLs as "text (url)") — a toggle; we keep link text
  only, which is the common "readable text" expectation.

**Out-of-model:** fetch-a-URL-then-strip (that's a separate fetch step; web-fetch
already exists and could feed this), bulk/batch.

## Tested
unit (5: strips-tags-no-markers, list items on lines, link text kept, no-CRLF,
empty-input error) + drift-guard · wafer fixtures (1) · `wafer build` · wasm-pack
web · generator · CLI (clean text with decoded `&amp;`) · Playwright page +
query-param deep-link (2 tests).

Note: html2text 0.17 was evaluated first but emits Markdown-ish decorations (`#`,
`**`) even via config::plain(); nanohtml2text gives genuinely plain output, so it
was used instead. (Recorded so a future pass doesn't re-try html2text for this.)

> Original work only — no competitor copy, branding, or trademarks copied.
