## About this tool

The Sitemap URL Extractor parses an XML sitemap and gives you a plain list of
every URL it contains. Paste the contents of a `sitemap.xml` file (a `<urlset>`
of page URLs) or a sitemap **index** (`<sitemapindex>`, a list of child
sitemaps) and the tool reads each `<loc>` element, pairing it with the
`<lastmod>` date when one is present.

It auto-detects which kind of document you pasted, handles XML namespace
prefixes, and is tolerant of minor formatting. The result is one URL per line,
with the last-modified date in a second tab-separated column when available —
ready to copy into a spreadsheet, a crawler, or a script.

Everything runs locally in your browser via WebAssembly. Your sitemap is never
uploaded to a server.

### Common uses

- Pull every page URL out of a site's `sitemap.xml` for an audit or migration.
- Expand a sitemap index into the list of child sitemaps to fetch next.
- Grab `lastmod` dates to prioritise re-crawling recently changed pages.
- Diff two sitemaps by extracting and comparing their URL lists.
