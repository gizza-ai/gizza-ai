# URL normalizer competitor scan (2026-08-23)

Tool: `url-normalizer` — bulk URL canonicalization for hosts, paths, percent-encoding, query ordering, and optional SEO cleanup.

## Competitors checked

1. Web-based URL parsers / canonical URL testers: common controls include entering a single URL, viewing normalized components, lowercasing scheme/host, removing default ports, and resolving path dot-segments. Most are inspect-first rather than bulk transform tools.
2. SEO canonical URL helpers: table-stakes behavior includes forcing HTTPS, choosing whether to keep or strip `www.`, removing directory index filenames, handling trailing slashes, removing fragments, and producing copyable canonical URLs.
3. Link-cleaning / URL-cleaner tools: table-stakes behavior includes stripping `utm_*` and click IDs, keeping useful query parameters, processing pasted URL lists, and showing before/after output.

Unreachable or brand-heavy pages were not copied; this file records paraphrased behavior and fit decisions only.

## Table-stakes decisions

| Capability | Competitor pattern | Decision |
| --- | --- | --- |
| Lowercase scheme and host | URL canonicalizers normalize `HTTP://Example.COM` to lowercase scheme/host. | In model; default behavior. |
| Remove default ports | Canonical URL tools drop `:80` on http and `:443` on https. | In model; default `strip_default_port=true`, with broader default-port table. |
| Resolve `.` and `..` path segments | RFC-style normalizers collapse `/a/b/../c` to `/a/c`. | In model; default `dot_segments=true`. |
| Percent-encoding canonical form | Parsers usually uppercase hex and decode unreserved escapes. | In model; `encoding=normalize`, with `decode` and `preserve` options. |
| Sort query parameters | Cache-key and SEO helpers sort query keys for stable comparison. | In model; `sort_query=key`, plus `key-value` and `none`. |
| Strip tracking parameters | Link cleaners remove `utm_*`, `fbclid`, `gclid`, `msclkid`, and similar IDs. | In model; optional `drop_tracking=false` by default. |
| Force HTTPS / HTTP | SEO canonical tools often offer an HTTPS canonical mode. | In model; enum `scheme=preserve|https|http`, only applied to web/scheme-less host URLs. |
| Add or strip `www.` | Canonical URL tools expose site policy for apex vs `www`. | In model; enum `www=preserve|strip|add`. |
| Directory index removal | SEO tools often convert `/index.html` to the directory URL. | In model; optional `drop_index`. |
| Trailing slash policy | Canonical helpers expose preserve/add/remove slash behavior. | In model; enum `trailing_slash`. |
| Relative URL resolution | URL resolvers accept a base URL to canonicalize relative references. | In model; optional `base`. |
| Bulk processing | Many link-cleaners accept pasted lists. | In model; one URL per line, 20,000 URL cap. |
| Before/after report | Audit tools show what changed. | In model; `output=report`, `changed`, and `summary`. |
| Custom query allow/deny lists | Advanced cleaners expose site-specific param filtering. | Out of model for this tool; sibling `url-query-normalizer` owns custom allow/deny lists. |
| Network validation, redirects, canonical tag fetching | SEO crawlers fetch URLs and inspect HTTP/HTML. | Out of model; gizza browser/CLI tool does not fetch user URLs. |
| Punycode/IDNA display policy | Some URL libraries convert internationalized domains. | Out of model for now; this pure implementation preserves host text except lowercasing and validation. |

## UX controls chosen

- Multiline textarea for bulk URLs.
- Text input for optional base URL.
- Select controls for scheme, `www`, percent-encoding, trailing-slash, query ordering, invalid-line handling, and result format.
- Checkboxes for independent cleanup toggles: default ports, dot-segments, repeated slashes, path lowercasing, index removal, duplicate query pairs, empty params, tracking params, fragments, and duplicate output URLs.
- Preset chips cover default RFC-style canonicalization, SEO cleanup, relative URL resolution, and CSV reporting.

## Worked examples used for verification

- `HTTP://Example.COM:80/a/b/../c?b=2&a=1` -> `http://example.com/a/c?a=1&b=2`.
- SEO cleanup with HTTPS, strip `www`, drop `index.html`, tracking and fragment -> `https://example.com/blog?id=42`.
- Relative `../images/logo.png` against `https://example.com/docs/guide/index.html` -> `https://example.com/docs/images/logo.png`.
