# extract-urls — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-urls` — extract all URLs from text, deduplicate, and
optionally split scheme/host/path components.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `grep -Eo 'https?://\S+'` | CLI | Standard, but grabs trailing `).` punctuation, doesn't validate, no dedup without `sort -u`, no component split. |
| Online "extract links from text" tools | Web | Common, but many upload your text, are IPv… er, ad-walled, and rarely split components or trim trailing punctuation cleanly. |
| Browser devtools / "copy all links" extensions | App | Work on rendered pages, not arbitrary pasted text. |
| Python `urlextract` / regex | Library | Need code; regex-only versions inherit the trailing-punctuation and validation problems. |

## How gizza's tool is better / different

1. **Validated, not just matched.** Each candidate is parsed by a real URL parser
   (`url` crate), so malformed strings are dropped — and the same parser powers
   the optional component split.
2. **Clean extraction.** Trailing prose punctuation is trimmed (`…/page.` →
   `…/page`) and bracket-wrapped URLs (`(https://x)`, `[http://y]`) come out
   without the brackets — the two things naive regex always gets wrong.
3. **Deduplicated, first-seen order.** No `sort -u` needed.
4. **Optional component breakdown.** One toggle splits every URL into scheme,
   host, port, path, query, and fragment — handy for auditing what a blob links
   to.
5. **Local + three surfaces.** Chat, CLI (`gizza tool extract-urls`), and a
   zero-upload page, one Rust core.

## Verification

CLI run on *"See https://example.com/a?x=1#top and (http://b.io:8080/p). Dup
https://example.com/a?x=1#top."* with `split_components=true` returned 2 unique
URLs (bracket stripped, duplicate collapsed) with correct component splits
(`b.io`, port `8080`, path `/p`; and scheme/host/path/query/fragment for the
first).

## Scope / honest limitations

- Matches **http/https** only by design (avoids the false-positive storm of bare
  domains, `ftp:`, `mailto:`, etc.). A future option could broaden schemes.
- Literal extraction, not link-text/anchor parsing (that's the HTML-table /
  readability family's job).

## Possible future enhancements

- Optional `www.`/bare-domain and other-scheme detection.
- Group by host (like extract-email-addresses' group-by-domain).
- Decode percent-encoding in the component view.
