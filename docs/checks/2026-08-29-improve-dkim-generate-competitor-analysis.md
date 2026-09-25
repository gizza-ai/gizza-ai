# Competitor analysis: dkim-generate

Date: 2026-08-29
Tool: `dkim-generate` — generate a DKIM key pair and the `<selector>._domainkey.<domain>`
TXT record that publishes its public half.

## Scan summary

Web search queries:
`DKIM record generator online tool key pair selector TXT` and
`DKIM key generator ed25519 RFC 8463 selector rotation tool features 255 character TXT split`.

Reviewed the feature shape of the mainstream DKIM record generators (MailSlurp, DKIM
Studio, EasyDMARC, PowerDMARC, SuperSend, AutoSPF, Sendmarc, CaptainDNS, Inveigle,
Sequenzy) plus the setup/rotation guidance those vendors publish. The category is very
uniform: enter a domain and a selector, pick RSA 1024/2048/4096 (the better ones also
offer Ed25519), and get back a private key PEM plus a `v=DKIM1; k=rsa; p=…` TXT record.
The differentiators are (a) whether generation is genuinely in-browser or server-side,
(b) whether the tool warns about the 255-character TXT string limit that RSA-4096 blows
through, and (c) whether anything other than "generate a fresh key" is possible.

No competitor copy, naming, or layout was reproduced; the page text here is original.

## Table-stakes found

| Capability / UX pattern | In-model decision | Implementation notes |
| --- | --- | --- |
| Domain input | Built | `domain`, normalized: a pasted URL, email address or `sel._domainkey.host` is reduced to the domain; punycode required for IDNs, with an explicit error otherwise. |
| Selector input | Built | `selector`, defaults to `mail`; validated as DNS labels (≤63 chars, no leading/trailing hyphen). |
| RSA 1024 / 2048 / 4096 choice | Built | `key_type` enum, `rsa-2048` default — the value every scanned tool recommends. |
| Ed25519 (RFC 8463) keys | Built | `key_type=ed25519`; `p=` is the base64 raw 32-byte public key per RFC 8463 §3, not an SPKI blob. |
| Keys generated locally, never uploaded | Built | Pure Rust/WASM on the page, `getrandom` → `crypto.getRandomValues`. Several competitors generate server-side; the page copy states the local-only guarantee plainly. |
| Private key as a downloadable/copyable PEM | Built | PKCS#8 PEM always; PKCS#1 (`BEGIN RSA PRIVATE KEY`) too for RSA, which is what OpenDKIM and several ESPs read. Ed25519 also returns the base64 32-byte seed OpenDKIM/rspamd store. |
| Public key PEM | Built | `output=public_key` returns the SPKI PEM. |
| Ready-to-paste DNS TXT value | Built | `output=dns_value` returns exactly `v=DKIM1; h=sha256; k=rsa; p=…`. |
| Host/name, type and TTL spelled out | Built | The default `text` report labels Host / Name, Type, TTL and Value, because DNS panels differ on whether the domain is appended. |
| 255-character TXT limit handling | Built | The value is chunked at 255 bytes; `output=zone_file` emits the multi-string BIND form, and an automatic note fires when the record needs more than one string. |
| Record-length / weak-key warnings | Built | Notes for <2048-bit RSA (weak), >2048-bit RSA (record length), and Ed25519 (incomplete receiver support → dual-sign advice). |
| `h=sha256` tag toggle | Built | `include_hash`, on by default. |
| `t=` flag tag (test mode, no-subdomains) | Built | `flags` enum `none`/`y`/`s`/`y:s`, with a note explaining that `t=y` suppresses enforcement. |
| Machine-readable output for scripting | Built | `output=json` returns domain, selector, key type/bits, the record (value, length, chunks, zone line), both key halves, and the notes array. |
| Selector-rotation guidance | Built (copy) | The page explains rotating onto a *new* selector rather than overwriting one, which is the M3AAWG-style advice the vendor blogs give. |
| Rebuild the record for a key you already have | Built — beyond most competitors | `existing_key` accepts PKCS#8, PKCS#1, a base64 Ed25519 seed, a public key PEM, or a bare `p=` value. Most scanned generators can only mint a new key. |
| Clear errors for unusable pasted keys | Built | Passphrase-encrypted, OpenSSH-format and certificate pastes each get a specific message naming the `openssl` conversion. |

## Out-of-model or deliberately rejected

| Feature | Reason |
| --- | --- |
| Look up / verify an existing published selector | Needs live DNS. This block is pure local compute with no network; a DNS-lookup tool is a separate block. |
| Publish the record to a DNS provider via API | Requires provider credentials and an account model this toolkit does not have. |
| Server-side generation with the key emailed or downloaded from a server | Rejected on principle — the private key must never leave the device; local WASM is the whole point. |
| Saved keys, dashboards, or rotation reminders | Outside the no-account/no-storage model. |
| SPF and DMARC record generation | Different records with different validation; separate tools, not extra modes here. |
| Signing messages or parsing `DKIM-Signature` headers | Verification/signing is a distinct job from key + record generation. |
| Decrypting passphrase-protected private keys | Would mean shipping a PBKDF/PKCS#5 path for a paste-time convenience; the error tells the user the one-line `openssl pkcs8` fix instead. |
| Optional `n=` (notes), `g=` (granularity) and `s=` (service) record tags | `g=` is deprecated and `s=`/`n=` are effectively unused in the wild; adding three more fields would cost more UX than it buys. |

## Resulting schema decisions

- `domain` is the only required param; `selector` defaults to `mail` so a one-argument
  call is meaningful.
- Fixed choices are enums so the page renders `<select>` menus and the chat schema
  constrains the model: `key_type`, `output`, `flags`.
- `include_hash` is the single boolean, defaulted to `true` (the recommended record).
- `existing_key` is a multiline string that accepts every paste form seen in the wild —
  private PEM, public PEM, bare base64 seed, or a `p=` value — rather than adding a
  separate "key format" selector the user would have to get right.
- Warnings ride along in the output (`notes`) instead of being a separate mode, so the
  weak-key and record-length advice is visible on every surface: chat, CLI and page.
