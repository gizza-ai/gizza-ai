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

## FAQ

<details>
<summary>I pasted a sitemap index — where are the page URLs?</summary>

A `<sitemapindex>` doesn't contain page URLs; it lists **child sitemaps**. The
tool detects the document kind and returns those child-sitemap URLs. Open each
one and paste its `<urlset>` content back in to get the actual page URLs.

</details>

<details>
<summary>Can I paste a compressed sitemap.xml.gz?</summary>

Not directly — the extractor parses XML text, so a gzipped file must be
decompressed first (`gunzip sitemap.xml.gz`, or just open the URL in your
browser, which usually decompresses it for you, and copy the XML).

</details>

<details>
<summary>Why do extracted URLs contain "&" where the file said "&amp;"?</summary>

`<loc>` values are XML-unescaped on the way out, so entities like `&amp;` come
back as the literal characters the URL really uses. That means the list is
directly usable in a crawler, spreadsheet, or script without further decoding.

</details>

<details>
<summary>Can it download the sitemap from my domain for me?</summary>

No — it runs entirely in your browser with no network access, which is also
why the sitemap content never leaves your machine. Fetch
`https://example.com/sitemap.xml` yourself (browser or `curl`) and paste the
XML here.

</details>
