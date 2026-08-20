## About this tool

`/blog` and `/blog/` are two different URLs. Servers usually redirect one to the other, but a list that mixes both — a sitemap, a redirect map, a crawl export, a link audit — produces duplicate rows, duplicate content warnings, and redirect chains that cost a hop on every request. This tool makes a whole list agree on one style.

Paste one URL per line, pick **add** or **remove**, and the trailing slash on every directory-style path is rewritten to match. Only the path is touched: the scheme, host, port, query string and fragment are copied through byte-for-byte, and nothing is re-encoded, so `?q=a%20b#top` comes back exactly as you typed it.

A worked example. With the default settings (`add`, file paths skipped, root normalized):

```
https://example.com/blog
https://example.com/blog/about/
https://example.com/sitemap.xml
https://example.com
```

becomes

```
https://example.com/blog/
https://example.com/blog/about/
https://example.com/sitemap.xml
https://example.com/
```

Line 2 was already correct, line 3 is a file and was left alone, and line 4 got the root slash every URL needs.

Switch the result to **Only the URLs that changed** to get just the rewritten lines — that is the redirect list to hand to your server config. **Per-line CSV report** returns `line,original,normalized,action` for every input line, where `action` is one of `added`, `removed`, `unchanged`, `root`, `skipped-file`, `invalid` or `duplicate`. **CSV summary of the totals** returns a `metric,value` count of each bucket.

What counts as a file path: a last segment whose extension is 1–10 alphanumeric characters containing at least one letter — `/sitemap.xml`, `/report.pdf`, `/style.css`, `/logo.svg`. That rule deliberately treats `/api/v1.2` and `/pricing` as directories. Turn the checkbox off to force every URL into the chosen style regardless.

Accepted line forms: absolute URLs (`https://host/path`, and any scheme with an authority such as `ftp://`), scheme-relative URLs (`//cdn.example.com/assets`), bare hosts (`example.com/blog`, `example.com:8080/blog`) and path-only lines (`/blog/post`).

Limits and edge cases:

- Up to 20,000 URLs and 1,000,000 bytes per run. Blank lines are ignored.
- Repeated trailing slashes collapse: `…/blog///` becomes `…/blog/` in add mode and `…/blog` in remove mode.
- The site root always keeps its single slash. `https://example.com` and `https://example.com//` both become `https://example.com/`, and remove mode never strips it — a bare `https://example.com` is not a shorter URL, it is an incomplete one. Uncheck the root option to leave root URLs exactly as written.
- Lines that aren't URLs — a note, a `mailto:` or `tel:` address, a stray word — are passed through untouched by default, so an annotated list survives a round trip. You can drop them instead, or make them stop the run.
- Nothing is fetched. The tool never checks which style your server actually serves, and it does not follow redirects or read status codes.
- Everything runs in your browser. The URLs are never uploaded, which makes staging and unpublished paths safe to paste.

## FAQ

<details>
<summary>Should I add or remove trailing slashes?</summary>

Either is fine for SEO as long as you pick one and stay consistent — search engines treat `/blog` and `/blog/` as separate URLs, so mixing them splits signals between two addresses. The practical constraint is your server: many static hosts and CMSes serve directory URLs with a slash and redirect the slashless form, while most frameworks do the opposite. Check what your site returns for one real URL, match that, and normalize the list to it.

</details>

<details>
<summary>Why is /sitemap.xml left unchanged?</summary>

Because `/sitemap.xml/` is a different resource on almost every server, and usually a 404. A trailing slash means "directory"; a path ending in a real file extension is a document. The tool leaves those lines alone in both directions and marks them `skipped-file` in the report. If your URLs genuinely need the slash anyway, uncheck "Leave file paths alone".

</details>

<details>
<summary>What happens to query strings and fragments?</summary>

They are preserved exactly, and the slash goes where it belongs — before the `?` or `#`. `https://example.com/blog?page=2#top` becomes `https://example.com/blog/?page=2#top` in add mode. Percent-encoding, parameter order and casing are never touched, so the only difference between input and output is the slash.

</details>

<details>
<summary>Can I get a list of just the URLs I need to redirect?</summary>

Yes — set the result to "Only the URLs that changed". You get the normalized form of every URL whose trailing slash actually moved, with the already-correct ones, the file paths and the unrecognized lines left out. If none changed, the tool says so instead of returning an empty box. For the before-and-after pairing, use the per-line CSV report, which lists the original and the normalized URL side by side.

</details>

<details>
<summary>Does it remove duplicates?</summary>

Only if you ask it to. Turn on "Drop duplicates after normalizing" and any URL that normalizes to something an earlier line already produced is dropped, keeping the first occurrence and the original order — that is what collapses a list containing both `/blog` and `/blog/` into one row. The summary output reports how many were removed.

</details>
