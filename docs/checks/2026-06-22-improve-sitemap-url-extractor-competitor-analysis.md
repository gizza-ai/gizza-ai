# sitemap-url-extractor — competitor analysis (2026-06-22)

Tool: parse an XML sitemap (`<urlset>`) or sitemap index (`<sitemapindex>`) and
extract every `<loc>` URL plus its optional `<lastmod>` date. Pure-Rust
(`quick-xml`), runs on all surfaces (chat / CLI / browser page), nothing uploaded.

## Top competitors surveyed

1. **SiteGPT — Sitemap URL Extractor** (sitegpt.ai/tools/sitemap-url-extractor) —
   fetch by URL, extract all URLs, free/no-signup.
2. **SEOwl — XML Sitemap URL Extractor** (seowl.co/sitemap-extractor) — URL-list
   extraction from XML sitemaps.
3. **SEOTesting — Free Sitemap URL Extractor** (seotesting.com) — paste a sitemap
   or index URL, auto-downloads extracted URLs as CSV.
4. **SEOBotAI — Free XML Sitemap URL Extractor** (seobotai.com) — instant
   extraction, marketed for SEO audits.
5. **Growthack.io — Advanced XML Sitemap URL Extractor** (growthack.io) — submit a
   file OR a URL, download results as CSV.
6. (also noted: contentforest.com, aubreyyung.com, chrisleverseo.com, searchant.co,
   browse.ai — same feature envelope.)

Sources:
- [SiteGPT](https://sitegpt.ai/tools/sitemap-url-extractor)
- [SEOwl](https://www.seowl.co/sitemap-extractor/)
- [SEOTesting](https://seotesting.com/free-seo-tools/sitemap-url-extractor/)
- [SEOBotAI](https://seobotai.com/tools/sitemap-url-extractor/)
- [Growthack.io](https://growthack.io/tools/extract-urls-from-sitemap-file/)
- [Content Forest](https://contentforest.com/tools/sitemap-url-extractor)

## Capability diff

| Capability | Competitors | gizza sitemap-url-extractor |
|---|---|---|
| Parse `<urlset>` `<loc>` URLs | yes | **yes** |
| Parse `<sitemapindex>` child-sitemap URLs | yes | **yes** (auto-detected; `kind` reported) |
| Extract `<lastmod>` per URL | partial (most show URLs only) | **yes** (tab-separated column on the page; structured field in chat/CLI) |
| Report URL count | some | **yes** |
| XML namespace prefix handling (`ns:loc`) | implicit | **yes** (explicit local-name match) |
| Paste raw XML | some (most are URL-fetch only) | **yes** (primary input) |
| Local / nothing-uploaded | no (all are server-side) | **yes** (WASM, in-browser) |
| Fetch sitemap by URL | yes | **out of model** — see below |
| Recursive index expansion (auto-fetch each child sitemap) | some (Growthack, SEOTesting) | **out of model** — requires network fetch |
| Gzipped (`.xml.gz`) sitemap input | some | **out of model** — page input is text paste, not a binary upload |
| CSV download | yes (SEOTesting, Growthack) | partial — output is copyable tab-separated text (URL\tlastmod), spreadsheet-pasteable |

## Gaps ranked (fit-to-model)

In-model, closed in this build:
- urlset + sitemapindex both supported with auto-detection and a reported `kind`.
- `<lastmod>` captured and surfaced (URL-only competitors don't), namespace-prefix
  tolerant, URL count, copy-pasteable tab-separated output.

Out-of-model (NOT built — deliberately, gizza tools are pure local compute):
- **Fetch-by-URL / recursive index expansion**: every competitor's headline feature
  is "give us a URL and we fetch + crawl the sitemaps". That needs server-side
  network I/O; gizza's page block is pure local WASM with no fetch, and the chat/CLI
  surfaces deliberately keep this a pure parser. The user fetches the XML (curl /
  browser view-source) and pastes it. Listed, not built.
- **Gzipped sitemap input**: the page takes a text field, not a binary upload, so
  `.xml.gz` decompression isn't wired here.
- **Built-in CSV file download**: the page renders text, not a file download; the
  tab-separated output pastes cleanly into a spreadsheet, covering the use case
  without a download button.

No competitor copy, branding, or trademarks were reproduced.

## Surfaces verified (2026-06-22)
- chat block: `wafer build` validates `target/block.wasm` (318.5 KiB).
- CLI: `gizza tool sitemap-url-extractor xml='<urlset>…'` → JSON
  `{"count":2,"entries":[…],"kind":"urlset"}`.
- page: Playwright `tool-page-sitemap-url-extractor.spec.ts` passes (urlset parsed,
  count + lastmod column rendered).
- unit tests: 7 passing (urlset, sitemapindex, namespace prefixes, empty/invalid
  input, render no-urls, render lastmod column) + chat-schema drift guard.
