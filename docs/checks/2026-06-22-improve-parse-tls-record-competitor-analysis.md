# parse-tls-record — competitor analysis (2026-06-22)

Tool: **TLS Record Parser** — decode TLS record-layer bytes (hex) into the record
header and, for handshake records, the handshake messages (ClientHello /
ServerHello fully decoded: version, random, session id, cipher suites by IANA
name, compression, and extensions — SNI, ALPN, supported_versions,
supported_groups, signature_algorithms, key_share). Alerts decoded to
level + description. Pure Rust → chat + CLI + page.

## Competitors surveyed

1. **CyberChef — "Parse TLS record" operation** (merged Feb 2025, gchq/CyberChef
   PR #1936). The closest direct competitor: a browser-local hex → TLS dissector.
   Decodes record types change_cipher_spec/alert/handshake/application_data and
   handshake types ClientHello, ServerHello, Certificate, CertificateRequest,
   CertificateVerify, NewSessionTicket. **Handles multiple sequential records in
   one input** and flags truncation. Notably it extracts ClientHello/ServerHello
   version/random/session-id/cipher-suites/compression but the source does **not**
   name cipher suites by IANA identifier and does **not** semantically decode the
   extension contents (SNI host, ALPN list, supported_versions).
2. **Wireshark / tcpdump** — the desktop reference dissector. Decodes everything,
   but requires installing software and opening a capture; not a paste-a-hex-blob
   web tool.
3. **rusticata/tls-parser & docs.rs/tls-parser** — Rust libraries (programmatic,
   not an end-user tool); used by analysts who write code.
4. **The Illustrated TLS 1.2 / 1.3 Connection (tls12/tls13.xargs.org)** —
   excellent educational byte-by-byte walkthroughs, but fixed example sessions,
   not a parser you feed your own bytes to.
5. **tls-hello-dump / tls-client-hello-stats (GitHub)** — CLI/PCAP scripts that
   pull ciphers + chosen suite from hellos; require Python/C tooling + a capture.

## Capability diff vs gizza parse-tls-record

| Capability | CyberChef | Wireshark | gizza (before) | gizza (after) |
|---|---|---|---|---|
| Browser-local, paste hex | yes | no (desktop) | yes | yes |
| Record header (type/version/length) | yes | yes | yes | yes |
| ClientHello / ServerHello fields | yes | yes | yes | yes |
| Cipher suites named by IANA id | **no** | yes | **yes** | yes |
| Extension contents: SNI / ALPN | **no** | yes | **yes** | yes |
| supported_versions / groups / sig-algs / key_share decoded | partial/no | yes | **yes** | yes |
| Alert level + description named | yes | yes | yes | yes |
| **Multiple sequential records in one input** | **yes** | yes | **no** | **yes** |
| Certificate / CertReq / CertVerify message bodies | yes | yes | no (typed only) | no (typed only) |
| chat / LLM-API surface | no | no | yes | yes |
| CLI surface | no | (tshark) | yes | yes |

## Gaps found and actions

- **Multi-record input (CLOSED).** CyberChef and Wireshark both parse a stream of
  concatenated records; the initial gizza build parsed only the first record. This
  is the highest-value in-model gap because real wire captures (e.g. ServerHello
  immediately followed by a Certificate record) are usually multi-record. Added
  `parse_records()` which walks the byte stream record-by-record; `run()` returns a
  single object for one record or a JSON array for several, and `render()` labels
  each `=== Record N of M ===`. Covered by unit tests + a Playwright test.
- **IANA cipher-suite names + decoded extension contents (already a gizza
  advantage).** gizza names cipher suites (TLS 1.3 suites + common TLS 1.2
  ECDHE/RSA suites) and decodes SNI host names, ALPN protocol ids,
  supported_versions, supported_groups, signature_algorithms, and key_share groups
  — which the CyberChef operation's source does not. Kept and emphasised in the
  page/skill copy.

## Deliberately NOT built (out of scope / lower value)

- **Full body decode of Certificate / CertificateRequest / CertificateVerify /
  NewSessionTicket.** gizza names these handshake types but does not decode their
  bodies. Full Certificate-chain decoding means an X.509 / DER parser, which is a
  separate concern (and a distinct tool); deferred rather than half-built.
- **Branding / copy / trademarks** from any competitor — none copied.

## Verification (all surfaces, 2026-06-22)

- `cargo test --workspace` — 15 core tests + 1 descriptor drift-guard pass.
- `wafer build` — block.wasm validates/instantiates (OK, ~332 KiB).
- `wasm-pack build .../web` — page wasm builds.
- CLI: `gizza tool parse-tls-record record=<ClientHello hex>` returns full JSON
  (cipher suites named, SNI=example.com, ALPN h2/http1.1, supported_versions);
  multi-record input returns a 2-element array.
- Page: `tests/tool-page-parse-tls-record.spec.ts` — 3 Playwright tests pass
  (ClientHello decode, multiple concatenated records, alert record).
