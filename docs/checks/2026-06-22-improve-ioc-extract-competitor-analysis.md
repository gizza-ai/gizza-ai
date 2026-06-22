# ioc-extract — competitor analysis (2026-06-22)

## What we built

`ioc-extract` extracts indicators of compromise from an arbitrary block of text and
groups them by type (de-duplicated + sorted):

- IPv4, IPv6 (full / compressed `::` / IPv4-mapped)
- URLs (http/https/ftp)
- Domains / hostnames (URL and email hosts excluded so categories don't overlap)
- Email addresses
- File hashes — MD5 (32), SHA-1 (40), SHA-256 (64), SHA-512 (128)

Inputs: `text` (required), `types` (comma-separated category selector or `all`), and
`defang` (re-defang the output). Defanged **input** is refanged automatically
(`hxxp[://]`, `1[.]2[.]3[.]4`, `bad[at]evil[dot]com`, plus `()` / `{}` bracket
variants and the `[dot]`/`[at]`/`hxxp` conventions).

Surfaces verified: chat block (`wafer build` validates instantiation), CLI
(`gizza tool ioc-extract …` — mixed extract, type filter, defang output, unknown-type
error), and the standalone page (Playwright: mixed extract + type-filter/defang).

## Top competitors surveyed

1. **InQuest/iocextract** (Python lib + CLI, the de-facto reference) —
   https://github.com/InQuest/iocextract
2. **mlab.sh — IOC Extractor Online** (client-side browser tool) —
   https://mlab.sh/tool/ioc-extractor
3. **malicialab/iocsearcher** (lib + CLI, also parses PDF/HTML/Word) —
   https://github.com/malicialab/iocsearcher
4. **Inventive HQ — IOC Extractor** (browser tool) —
   https://inventivehq.com/tools/security/ioc-extractor
5. **iocextract docs / PyPI** — https://pypi.org/project/iocextract/

## Gap analysis (fit-to-model)

| Capability | Competitors | ioc-extract | Decision |
|---|---|---|---|
| IPv4 / IPv6 | all | yes | covered |
| URLs | all | yes | covered |
| Domains / hostnames | most | yes (de-overlapped) | covered |
| Email addresses | all | yes | covered |
| MD5 / SHA-1 / SHA-256 | all | yes | covered |
| SHA-512 hashes | iocextract | yes | covered (many browser tools stop at SHA-256) |
| Recognize defanged INPUT | iocextract, iocsearcher, mlab | yes | covered |
| Re-defang the OUTPUT | iocextract (`--refang` is the inverse) | yes (`defang=true`) | covered |
| Per-type selection | iocextract (`--extract-ips` etc.) | yes (`types=`) | covered |
| De-dup + grouped output | mlab, InventiveHQ | yes | covered |
| Runs 100% client-side / private | mlab, InventiveHQ | yes (page = wasm) | covered |

### Gaps deliberately NOT built (out of scope / out of model)

- **YARA-rule extraction** (iocextract `--extract-yara-rules`): a distinct
  parsing problem and outside this tool's stated scope (IPs/URLs/domains/emails/hashes).
  Better as a separate tool if ever wanted.
- **CVE IDs, CIDR ranges, ASN, bitcoin/crypto-wallet addresses, registry keys,
  file paths, MAC addresses** (iocsearcher's wider catalogue): each is a small
  additive scanner but expands the tool well past its name/description; left out to
  keep the surface focused. Candidates for a future `--types` extension if requested.
- **PDF / HTML / Word file input** (iocsearcher): gizza pages take a text field, not
  a binary document upload, for this tool; out of the page model. The text surface
  already covers pasting extracted report text.
- **Custom regex** (iocextract `--custom-regex`): not meaningful for an LLM-tool /
  fixed-page surface.

## Improvements applied vs. a naive first build

- Defanged-input refang is built in (so pasting straight from a CTI report works) —
  matched to the iocextract/iocsearcher baseline, not just plain regex extraction.
- `defang=true` re-defangs the OUTPUT (the inverse of iocextract `--refang`) so the
  result list is itself safe to paste into a ticket.
- Domains are de-overlapped from URL/email hosts so the same host isn't listed in
  three categories (a common annoyance in naive extractors).
- IPv4 false-positive guard: rejects 3-octet version strings and leading-zero octets
  (e.g. `1.2.3`, `01.02.03.04`) that dumb `\d+\.\d+\.\d+\.\d+` regexes over-match.
- SHA-512 supported (several browser competitors stop at SHA-256).

## Result

Feature parity with the mainstream browser IOC extractors (mlab.sh, Inventive HQ) and
the core of the iocextract CLI, within gizza's pure-Rust / text-input model. The only
unmatched competitor features are out-of-scope (YARA, document parsing, custom regex)
or additive type expansions noted above.

NOTE: No competitor copy, branding, or trademarks were reproduced.
