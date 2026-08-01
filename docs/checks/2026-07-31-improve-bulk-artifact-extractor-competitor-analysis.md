# bulk-artifact-extractor — competitor analysis (2026-07-31)

Scan before implementation. Goal: extract common artifacts from a pasted text/blob and report kind, exact value, byte offset, and context. Notes are paraphrased from public tool behavior and documentation; no competitor copy or branding reused.

## Competitors / references inspected

1. **bulk_extractor-style forensic scanners** — scan disk images/files for emails, URLs, domains, phone/card-like strings, and report offsets or feature files for triage.
2. **CyberChef / extractors** — browser-local recipes for extracting URLs, IPs, email addresses, domains, cryptocurrency addresses, and other indicators from pasted text.
3. **grep/ripgrep + IOC extractor scripts** — command-line workflows that emit structured hit lists from logs or strings output; often include context and allow kind-specific filters.

## Table-stakes mapped to this tool

| Capability | In model? | Decision |
|---|---:|---|
| Email extraction | yes | Regex with domain/TLD checks; suppresses nested domain. |
| URL extraction | yes | Detects `http(s)://` and `www.` URLs; trims trailing punctuation. |
| IPv4 extraction | yes | Octets range-checked to 0–255. |
| Bare domain extraction | yes | Finds dotted hostnames with alphabetic TLDs outside email/URL spans. |
| Phone-number extraction | yes | Heuristic grouped-digit pattern, digit-count checked. |
| Credit-card extraction | yes | 13–19 digit candidates must pass Luhn. |
| Bitcoin address extraction | yes | Common legacy base58 and bech32-looking addresses. |
| Byte offsets | yes | Reports byte offset in the pasted UTF-8 input. |
| Context snippets | yes | Configurable context width with newlines flattened. |
| Kind filters | yes | `all` or comma-list of supported kinds. |
| JSON output for pipelines | yes | `output=json` emits an array of objects. |
| Direct binary/disk-image parsing | no | Current pure page/chat model takes UTF-8 text, not arbitrary binary file streams. Listed as a limit; users can paste strings output. |
| Unicode-aware global phone validation | no | Requires country metadata; heuristic extraction is documented. |
| Blockchain/card issuer validation | no | Out-of-model network lookups; this is local pattern + checksum triage only. |

## UX decisions applied

- Multiline paste box with example chips for all-artifact table output, kind-filtered JSON, and capped short-context review.
- Sliders for `context` and `limit` to match the generator's supported numeric controls.
- Explicit warning that offsets refer to the pasted text, not an original binary image after external strings extraction.
- JSON output included from the start for downstream incident-response pipelines.
