# ssh-public-key-parser competitor analysis — 2026-08-13

## Scope

Tool: `ssh-public-key-parser` — parse OpenSSH public keys, `authorized_keys` and `known_hosts` entries into algorithm, key size, comments and fingerprints.

This scan was used to decide the first descriptor/page shape before implementation. Notes are paraphrased; no competitor copy, branding or UI assets were copied.

## Competitors reviewed

| Competitor | Table-stakes observed | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- |
| OpenSSH `ssh-keygen -l/-E` | Local fingerprint display; SHA256 default, MD5/SHA1 legacy algorithms; key bit size; comment; certificate-aware fingerprint semantics. | Match `ssh-keygen -l` SHA256 and MD5 strings exactly; add optional SHA1; report key size, comment, certificate validity metadata and fingerprint of the certified key rather than the certificate envelope. | Running local filesystem commands and key generation are outside this parser page; users can still compare output against local `ssh-keygen`. |
| Browser SSH public-key decoders / key info pages | Paste a single public key; detect algorithm; show fingerprints; simple copy/paste text area; reject malformed base64. | Browser-local text area with no upload; exact JSON output; clear errors for malformed base64, PEM public keys and private keys; worked example and preset chips. | Server-side storage, accounts, key inventories and sharing links are intentionally not built. |
| Authorized-keys / known-hosts inspection workflows | Operators often paste whole `authorized_keys` or `known_hosts` files; options prefixes, host patterns, `@cert-authority` and hashed `|1|` hosts need separate reporting. | Parse multiple lines, ignore blank/comment lines, split `authorized_keys` options, report host patterns/hashed-host markers and known-host markers. | Dehashing `|1|` known_hosts entries is cryptographically not possible without testing a candidate hostname; this tool reports that the host is hashed. |
| SSH certificate inspection references | Certificates expose serial, type, key id, principals, validity windows, critical options/extensions and CA key details. | Parse OpenSSH `*-cert-v01@openssh.com` blobs; report serial/type/key id/principals/validity/status/options/extensions/CA fingerprint; page explains certificate fingerprint semantics. | Certificate signing/verification against an external CA trust store is not a pure pasted-key parser feature and was not built. |

## Parameter and UX decisions

- `input` (required multiline text): accepts one or many public-key lines, authorized_keys lines, known_hosts entries, OpenSSH certificates and RFC 4716 blocks.
- `expected_fingerprint` (optional text): verifies against pasted SHA256, MD5, SHA1 or bare fingerprint forms without forcing users to normalize punctuation/case.
- `include_sha1` (checkbox, default false): legacy compatibility without cluttering the default report.
- `uppercase_md5` (checkbox, default false): matches older consoles/inventory tools that display MD5 in uppercase.
- Page examples cover Ed25519, weak RSA, authorized_keys options, fingerprint verification and legacy fingerprint display.

## Fit-to-model assessment

Everything shipped is pure Rust, deterministic and browser-local. Private-key parsing was deliberately rejected even though it is technically parseable: accepting private keys in a web page trains unsafe behavior and is unnecessary for a public-key parser. PEM public keys are also rejected with conversion guidance because OpenSSH public-key blob semantics, comments and known-host formats are the tool's model.

## Verification plan

- Unit tests for happy paths, malformed input, multiple keys, authorized_keys, known_hosts, RFC 4716, certificates, fingerprint comparisons and size/count caps.
- CLI exact-output checks against the Ed25519 fixture and legacy checkbox paths.
- Playwright page tests for real JSON output, deep-link query params, checkbox states, weak-key warnings and private-key error messaging.
