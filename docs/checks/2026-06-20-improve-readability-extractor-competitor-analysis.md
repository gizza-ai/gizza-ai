# readability-extractor — competitor analysis (2026-06-20)

Fifteenth `/create-next-tool` backlog pick. Pure-Rust (dom_smoothie — a Mozilla
Readability port) + nanohtml2text for clean text. Pure-text-with-page tool (all 3
surfaces). Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| Mercury/Postlight, Readability.js | score blocks, strip nav/ads, return title + clean article HTML/text | capabilities |
| browser "reader mode" | declutter to article; font/theme controls | UX |
| various "extract article" tools | paste URL or HTML; output text or HTML | capabilities |

## Gap diff vs our tool
Our tool: runs a Readability-style extraction over pasted HTML, returns the title
+ main body as clean plain text (default) or cleaned article HTML, stripping nav,
ads, and boilerplate. Covers the core extraction + both output formats.

**Improvement made this pass:** text mode pipes the cleaned article HTML through
nanohtml2text so blocks are properly separated (dom_smoothie's raw `text_content`
runs adjacent heading/paragraph blocks together) — verified the output is now
`Headline\n\n<body>` rather than run-on text.

**In-model gaps considered, deferred:**
- **Markdown output** — a third `format` (the article as Markdown) could reuse the
  htmd crate from html-to-markdown; easy follow-up.
- **Fetch-by-URL** — the row is about pasted HTML; fetching is a separate step
  (web-fetch exists and can feed this). Out of scope here by design.
- **Author/date/excerpt metadata** — dom_smoothie exposes some (byline, excerpt);
  could be surfaced as extra fields.

**Out-of-model:** reader-mode font/theme UI (a viewer concern, not a tool),
site-specific extraction rules.

## Tested
unit (3: extracts article text + drops nav/ad/footer, html mode returns markup,
empty-input error) + drift-guard · wafer fixtures (1) · `wafer build` validates
the block (pure-Rust → also works in the chat SW) · wasm-pack web · generator ·
CLI (clean `Title\n\nbody`, chrome stripped) · Playwright page + query deep-link
(2 tests). NOTE: `readable-article` (the next backlog row) is a near-dup of this —
it will be skiplisted.

> Original work only — no competitor copy, branding, or trademarks copied.
