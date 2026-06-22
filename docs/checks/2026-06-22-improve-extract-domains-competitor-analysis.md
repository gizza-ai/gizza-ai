# extract-domains — competitor analysis (2026-06-22)

## What gizza extract-domains does

Scans arbitrary text and extracts the domains it references — from plain text,
http(s)/ftp URLs, and email addresses. Every candidate is validated against
Mozilla's **Public Suffix List** (via the `psl` crate's built-in compiled list),
so IP addresses, version numbers (`3.14`), and bogus TLDs are dropped. Results
are deduplicated.

Params:

- `text` (required) — input.
- `mode` = `hostname` | `registrable` | `both` (default `both`) — full hostnames
  (`www.blog.example.co.uk`), registrable domains / eTLD+1 (`example.co.uk`), or
  both lists with counts.
- `sort` (boolean, default false) — alphabetical vs first-seen order.

Three surfaces verified: chat block (`wafer build` validates + instantiates the
2.3 MB wasm — the PSL table instantiates fine in wasm32-wasip1), CLI
(`gizza tool extract-domains …`), and the standalone page (Playwright, 3 specs).

## Competitors surveyed

- link-grabber.com/tools/domain-extractor — "URL to registrable domain"
- goforpost.com/tools/domain-extractor
- toolskit.cc/tools/domain-extractor — "URLs, Emails & Text"
- ipvoid.com / apivoid.com domain-extractor
- phrasefix.com, wmtools.me, everydaytools.pro, urltodomainextractor.com

## Feature comparison

| Feature | Competitors | gizza extract-domains |
| --- | --- | --- |
| Extract from URLs, emails, plain text | yes (most) | yes |
| Hostname vs registrable (eTLD+1) toggle | yes (better tools) | yes (`mode`) |
| PSL-aware multi-level suffix (`co.uk`) | best tools only | yes |
| Deduplicate | yes | yes (first-seen) |
| Drop IP addresses | some ("remove IPs" option) | yes (PSL rejects them) |
| Sort alphabetically | some | yes (`sort`) — added in this pass |
| 100% client-side / private | some | yes (wasm in-browser; nothing uploaded) |
| Counts | some | yes (hostname_count + registrable_count) |

## Gaps closed in this pass

- **Alphabetical sort** — competitors commonly offer sorted output; added a
  `sort` boolean (default false = first-seen order) across core, chat schema,
  CLI, and page.

## Out-of-model / deliberately not built

- **File upload (.txt/.csv/.log)** — gizza's page input model is a paste field;
  file-to-text upload for a pure tool isn't part of the page driver. Pasting
  covers the same need.
- **TLD-length / extension filtering** (e.g. "only 2–3 letter TLDs") — niche
  domain-flipping feature; not generally useful and would clutter the schema.
- **Bulk export / CSV download** — the page renders text output a user can copy;
  no download surface for pure-text tools.

## Notes

- No competitor copy, branding, or trademarks were used.
- `psl` applies an implicit `*` wildcard rule, so it returns Some even for bogus
  single-label TLDs; we additionally require the matched suffix's `typ()` to be a
  known ICANN/private entry, which is what correctly rejects `3.14`, IP
  fragments, and made-up TLDs.
