# link-extractor — competitor analysis (2026-06-21)

New tool: **link-extractor** — extracts every hyperlink and in-page anchor (jump
target) from pasted **HTML or Markdown**. Pure-Rust (`scraper` for HTML,
`pulldown-cmark` for Markdown, `url` for base resolution) → runs on all backends
(chat / CLI / page), nothing uploaded.

## Surfaces verified

- **Chat / LLM API** — `wafer build` validates + instantiates the block (OK,
  1450.9 KiB); `wafer test` fixture (`html.json`) passes. Schema drift-guard test
  green.
- **CLI** — `gizza tool link-extractor input=… source=… base_url=… dedup=…`
  returns the structured JSON report (links with `url`/`text`/`relative`/`rel`,
  plus anchors). Verified HTML, Markdown, auto-detect, base-URL resolution,
  dedup, and `rel` capture.
- **Page** — `/tools/link-extractor/`; 3 Playwright specs pass (HTML
  links+anchors, base-URL resolve + dedup via the checkbox, Markdown deep-link →
  JSON).

## Competitors surveyed

| Tool | Input | Extracts | Notable features |
|------|-------|----------|------------------|
| [Geekflare Link Extractor](https://geekflare.com/tools/link-extractor/) | URL/HTML | href, anchor text, **rel** (nofollow/sponsored/ugc/dofollow), element type | internal/external/mailto **classification**, **CSV** export, filter by category |
| [GetTextTools Hyperlink Extractor](https://www.gettexttools.com/hyperlink-extractor/) | HTML/text | anchor text + href | **relative→absolute via base URL**, Excel export |
| [PhraseFix Link Extractor](https://phrasefix.com/tools/extract-links-and-anchors/) | HTML | href, text, anchor, **class**, **id** | file import, copy/share |
| [Browserling / FromDev](https://www.browserling.com/tools/extract-urls) | text/HTML | URLs | **unique-link dedup**, plain URL list, client-side |
| [MarkdownMe Link Extractor](https://markdownme.com/tools/link-extractor) | Markdown | URLs from MD | client-side, no signup |

## Gap analysis → what was closed (all in-model, pure, no network)

Started with: HTML + Markdown parsing, link `url`+`text`, `relative` flag,
anchors (id/name/heading slugs), `auto` format detection, text + JSON output.
Closed the differentiated competitor gaps:

1. **`rel` attribute** (Geekflare's standout SEO signal) — each HTML `<a rel>` is
   now captured per link (`nofollow`, `sponsored`, `ugc`, `noopener`, …) and shown
   in both text (`[rel: …]`) and JSON.
2. **Base-URL relative→absolute resolution** (GetTextTools) — optional `base_url`
   param resolves every relative link to its absolute form via the WHATWG `url`
   crate; absolute / fragment / `mailto:` links are left untouched.
3. **Dedup** (Browserling/FromDev "unique links") — optional `dedup` flag collapses
   links sharing the same final URL, first-seen kept.

### Differentiators gizza already had / kept

- **Markdown is a first-class input** alongside HTML (most competitors do one or
  the other); autolinks, reference links, and heading-slug anchors are handled.
- **In-page anchors / jump targets** (`id`, legacy `<a name>`, GitHub-style
  heading slugs) — most "link extractors" only list outbound hrefs.
- Pure-Rust, runs identically in chat, CLI, and the page; nothing uploaded.

## Out-of-model / deliberately not built

- **Live URL fetch** (Geekflare/HackerTarget/Apify crawl a page by URL): gizza's
  pure block takes pasted markup — fetching is a separate network concern
  (covered by `css-select-extract` / `web-fetch`). Not added here.
- **internal/external split by the page's own domain**: requires knowing the
  document's own URL; partially covered by the `relative` flag + `base_url`
  resolution. A dedicated `internal`/`external` classification keyed on a host was
  left out to keep the schema lean (the resolved absolute URL makes host-grouping
  trivial downstream).
- **CSV/Excel export**: the JSON output is the structured, pipeable form; CSV is a
  thin downstream transform (the `csv-*` tools cover it). Not duplicated here.
- No competitor copy, branding, or trademarks were used.
