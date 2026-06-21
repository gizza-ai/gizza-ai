# extract-email-addresses — competitor analysis & differentiation

**Tool:** `gizza-ai/extract-email-addresses` — pull all email addresses out of
text, deduplicate, optionally group by domain.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| "email extractor" web tools (emailextractor.io, etc.) | Web | Common, but most **upload your text to a server** (bad for contact lists / threads), and many are ad-walled or rate-limited. |
| Spreadsheet / regex in an editor | DIY | Works, but you write/maintain the regex, then dedupe by hand; no domain grouping. |
| `grep -Eo '<regex>' file \| sort -u` | CLI | Fine for the unix-comfortable; no domain grouping, no first-seen ordering, regex quality varies. |
| Marketing "email scrapers" | App | Aimed at scraping websites; heavyweight and often spammy; overkill for "extract from this text". |

## How gizza's tool is better / different

1. **Local, private.** Runs in WASM (chat / CLI / page) — your text never leaves
   the device. The decisive advantage over upload-based web extractors when the
   input is a contact list or private thread.
2. **Deduplicated + ordered.** Case-insensitive de-duplication with first-seen
   ordering, out of the box.
3. **Group by domain.** One toggle buckets the addresses by domain (and counts
   them) — useful for seeing which organizations appear.
4. **Three surfaces, one core.** Chat ("grab the emails from this"), CLI
   (`gizza tool extract-email-addresses`), and a zero-upload page.

## Verification

CLI run on *"Email alice@corp.com or bob@corp.com. Also Carol@OTHER.io and
alice@corp.com again."* with `group_by_domain=true` returned 3 unique addresses
(deduped `alice@corp.com`, case-folded `Carol@OTHER.io` → `carol@other.io`) and
the per-domain grouping `corp.com → [alice, bob]`, `other.io → [carol]`.

## Scope / honest limitations

- Pragmatic matcher (local-part `@` domain with a real TLD); it won't match
  exotic quoted/IDN local-parts, by design, to avoid false positives.
- Addresses are lowercased for de-duplication (the local-part is technically
  case-sensitive per RFC, but real-world providers treat it case-insensitively).

## Possible future enhancements

- Optional obfuscation handling ("alice [at] corp [dot] com").
- Output sorted alphabetically, or counts-per-domain only.
- A companion extract-urls / extract-phone-numbers family.
