# article-to-epub — competitor analysis (2026-08-15)

Scan run **before** implementing, per `create-next-tool`. One web search
("online tool convert article text or HTML to EPUB ebook free"), then the top
three real competitor pages were fetched and skimmed. Everything below is a
paraphrased feature inventory — no competitor copy, wording, or branding is
reused anywhere in the tool.

## Competitors skimmed

| # | Tool | Reachable | Notes |
|---|------|-----------|-------|
| 1 | aconvert.com — "HTML to EPUB" | yes | Calibre-backed converter: base font size, font embedding, page margins, editable metadata, "remove first image" cover fix, batch, cloud save |
| 2 | ebook.online-convert.com — "Convert to EPUB" | yes | Richest option set: title + author metadata, base font size (pt), EPUB 2 vs 3 selector, border/margin in cm, input-encoding override, heuristics toggle, 40+ target-device profiles, 30+ input formats |
| 3 | convertio.co — "HTML to EPUB" | yes | Zero conversion options; upload / URL / Drive / Dropbox in, 1 GB cap, account + paid tiers, API |

All three are **server-side upload converters** (file in → job → download). None
of them packages *pasted article text* locally in the browser, which is the shape
this tool takes.

## Table-stakes inventory

| Capability | Seen on | In model? | Where it landed |
|---|---|---|---|
| Article/HTML in → valid EPUB out | 1, 2, 3 | in | `core::convert` → EPUB 3 OCF zip (`mimetype` stored first, `META-INF/container.xml`, `content.opf`, `nav.xhtml`, `toc.ncx`) |
| Paste text (not just upload a file) | none (all three need a file/URL) | in | `content` is a multiline field — **our differentiator**, plus it is the whole point of the backlog row |
| Title metadata | 1, 2 | in | `title` (auto-derived from `<title>`/first heading/first line when blank) → `dc:title` |
| Author metadata | 1, 2 | in | `author` → `dc:creator` |
| Publisher / extra metadata | 1 ("metadata editable") | in | `publisher` → `dc:publisher` |
| Language metadata | implied by 1's metadata editor | in | `language` (BCP-47, default `en`) → `dc:language` + `xml:lang` |
| Base font size | 1, 2 | in | `base_font_size` in pt (0 = leave it to the reader) → embedded `style.css` |
| Table of contents / chapter structure | implied by all (Calibre generates one) | in | `split_level` (`none`/`h1`/`h2`/`h1-h2`) splits at headings and builds both an EPUB 3 nav doc and an EPUB 2 NCX |
| Title page | 2 (via metadata) | in | `include_title_page` (default on) writes a title/author/publisher page as the first spine item |
| Sensible reading styles (margins, line height, wrapped code) | 1 (margins), 2 (border) | in | embedded `style.css` shipped in every book |
| EPUB 2 vs EPUB 3 selector | 2 | **considered, rejected** | Every book already ships EPUB 3 markup **and** an EPUB 2 `toc.ncx`, so it opens in EPUB 2-era readers as-is; a version switch would be schema bloat for no reachable behaviour difference |
| Font embedding / subsetting | 1, 2 | **out** | Needs font binaries the browser doesn't have; readers substitute their own fonts anyway |
| Target-device profiles (Kindle, Kobo, …) | 2 | **out** | Device profiles mostly drive MOBI/AZW3 output and margin heuristics in a server-side Calibre pipeline; EPUB is reflowable by design |
| Cover image | 1 (first-image removal implies covers) | **out** | The page form takes text, not an image upload; a cover needs binary image input a pure text tool has no channel for |
| Images carried into the book | 1, 2 | **out (degraded gracefully)** | Remote `<img>` can't be packaged locally; images are dropped, their `alt` text is kept inline as `[Image: …]`, and the dropped count is reported |
| Fetch an article by URL | 3, 2 | **out** | Network fetch is a different block family (`web-fetch`); this tool stays offline and pure. Chain `web-fetch` → `article-to-epub` in chat |
| Batch / multi-file conversion | 1, 3 | **out** | One article per run; the CLI covers scripted batching |
| Cloud save (Drive/Dropbox) | 1, 3 | **out** | No accounts, no server — the file downloads straight from the page |
| Other output formats (MOBI, AZW3, FB2…) | 1, 2 | **out** | Separate backlog rows; this row is EPUB |
| Nothing uploaded / private | none (all upload to a server) | in (free) | Runs entirely in the browser wasm — **our second differentiator** |

Nothing from the scan was dropped silently: every row is either implemented or
listed as out-of-model / rejected above.

## Design decisions taken from the scan

- **Paste-first, not upload-first.** All three competitors are file converters;
  the common real job ("I have the cleaned article text/HTML, give me an EPUB")
  needs a textarea, so `content` is the primary input and the page is a paste box.
- **Structure beats knobs.** Instead of 40 device profiles, the one setting that
  changes the reading experience most is chapter splitting, so `split_level` is
  first-class and drives a real, navigable TOC in both nav formats.
- **Well-formed XHTML is enforced, not assumed.** Pasted article HTML is usually
  tag-soup (unclosed `<p>`, `&nbsp;`, `<script>` leftovers). The converter
  re-emits an allowlisted, stack-balanced XHTML subset and decodes named entities
  to real characters, because an EPUB whose XHTML doesn't parse fails to open in
  strict readers.
- **Deterministic output.** Fixed ZIP timestamps and a content-derived
  `dc:identifier` mean the same input always produces byte-identical bytes —
  testable, diffable, and cache-friendly. No competitor offers this.
- **Stated caps.** 2,000,000 input characters and 2,000 chapters, both on the
  page. Two competitors state no limit at all; convertio states 1 GB.

## Not copied

No competitor copy, FAQ wording, option labels, branding, or trademark appears in
`page/meta.toml` or `page/content.md`. The EPUB structure follows the public
EPUB 3 / OCF specifications, and the XHTML allowlist was written for this tool.
