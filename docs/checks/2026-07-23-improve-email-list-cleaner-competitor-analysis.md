# email-list-cleaner — competitor analysis (2026-07-23)

Scan of the top free "email list cleaner / list hygiene" tools before building
`blocks/email-list-cleaner`. All findings are paraphrased from public product
pages; no competitor copy, branding, or trademarks are reproduced. The goal is to
enumerate table-stakes features and tag each **in-model** (pure-Rust, offline —
buildable here) or **out-of-model** (needs network/DNS/SMTP or an ML model —
listed, not built, since gizza blocks are pure Rust + ffmpeg with no host I/O).

## Tools surveyed

- Jitbit Email List Cleaner — paste one-per-line, removes invalid mailboxes,
  typos, duplicates, malformed addresses; markets in-browser processing.
- Sidemail Email List Cleaner — invalid removal, disposable-domain flag, typo
  catch, optional SMTP handshake verification.
- MailDiver Email List Cleaner — RFC 5321 syntax validation (format, domain
  structure, length limits), trims whitespace, lowercases, standardizes case,
  removes duplicates; CSV upload up to a few MB; client-side.
- Mailmeteor Remove Duplicates — paste list, dedupe with an "ignore
  capitalization" toggle.
- ListWrangler / Bizzuppoter text+list cleaners — paste one-per-line, remove
  duplicates (case-insensitive option), trim, format case, export; all in-browser.
- Clearout / ZeroBounce (premium) — real-time verification, disposable + spam-trap
  + inactive-mailbox detection (network/SMTP, paid).

## Table-stakes → model-fit decisions

| Capability | Fit | Where it lands |
| --- | --- | --- |
| Paste a multiline list (one address per line) | in-model | `emails` textarea input |
| Accept comma- and semicolon-separated lists (Outlook/CSV paste) | in-model | core splits on newline `,` `;` |
| Trim surrounding whitespace | in-model | via reused email-normalizer wrappers |
| Lowercase / case-normalize | in-model | default; canonical lowercased form |
| Strip `mailto:` and `Name <addr>` wrappers | in-model | reused email-normalizer `strip_wrappers` |
| RFC 5321/5322 syntax validation (format, local/domain, length caps) | in-model | reused email-validator `validate` |
| Flag malformed / invalid entries with a reason | in-model | invalid-row report section |
| Typo detection + suggestion (`gmial.com` → `gmail.com`, `.con` → `.com`) | in-model | reused email-validator suggestions → "Possible typos" section |
| De-duplicate, case-insensitive | in-model | dedupe on cleaned key |
| Preserve first-seen order (don't reshuffle) | in-model | default `sort = input` |
| Optional alphabetical sort | in-model | `sort = alpha` |
| Gmail-style alias canonicalization (drop dots + `+tag` so aliases collapse) | in-model | `canonicalize = true` (reused email-normalizer canonical form) |
| Counts: rows processed, valid unique, duplicates removed, invalid | in-model | report summary |
| Copy-ready clean list output | in-model | `format = clean` (newline) + built-in Copy button |
| Comma-joined output (for pasting back into a To: field) | in-model | `format = comma` |
| Full report (summary + valid + invalid + typos) | in-model | `format = report` (default) |
| MX / DNS record check | out-of-model | needs DNS; syntax-only here — stated as a page limit |
| SMTP mailbox verification (does it actually receive mail) | out-of-model | needs network SMTP; stated as a limit |
| Disposable / throwaway-domain detection | out-of-model here | covered by the separate `disposable-email-detector` tool; cross-referenced |
| Role-based address flag (info@, sales@) | out-of-model here | deferred; not a cleanup/validity concern, kept for a future pass |
| CSV file upload | out-of-model here | the page is a paste-in text tool; paste covers the same input |

## Design summary

Rather than re-implement address parsing, the core **reuses two existing block
cores** as path dependencies:

- `email-validator` decides validity (authoritative) and yields the typo
  suggestion for each row.
- `email-normalizer` produces the cleaned (trim + lowercase) form and — when
  `canonicalize = true` — the provider-canonical form (Gmail dot/`+tag`
  folding), which becomes the de-duplication key.

Params (all with `.describe()`, fixed choices as `Param::enumv`):
`emails` (required list), `canonicalize` (bool, default false),
`sort` (`input` | `alpha`, default `input`), `format` (`report` | `clean` |
`comma`, default `report`).

Out-of-model network verification (MX/SMTP) and disposable detection are stated
as limits on the page and, where a sibling tool already covers them, cross-linked
generically — never silently dropped.

## Sources

- https://www.jitbit.com/listcleaner/
- https://sidemail.io/tools/email-list-cleaner/
- https://maildiver.com/tools/email-list-cleaner/
- https://mailmeteor.com/tools/remove-duplicates
- https://listwrangler.app/remove-duplicates/
- https://clearout.io/email-list-cleaner/
- https://www.zerobounce.net/
