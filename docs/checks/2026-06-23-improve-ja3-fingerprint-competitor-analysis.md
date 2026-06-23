# ja3-fingerprint — competitor analysis (2026-06-23)

Tool: **JA3 Fingerprint Calculator** — paste a TLS ClientHello as hex, get the
JA3 fingerprint string + MD5 (and JA3N). Pure-Rust, runs on all backends
(chat / CLI / page).

## What the tool does

Parses a ClientHello (optionally prefixed by the TLS record header `16 03 ...`
and/or the handshake header `01 ...`), removes GREASE values (RFC 8701), and
builds the JA3 string:

```
SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats
```

then MD5-hashes it. Returns: `ja3`, `ja3_md5`, `ja3n`, `ja3n_md5`,
`tls_version`, `ciphers[]`, `extensions[]`, `elliptic_curves[]`,
`elliptic_curve_point_formats[]`, `server_names[]` (SNI).

## Top competitors surveyed

1. **Scrapfly — JA3/JA4 TLS Fingerprint** (scrapfly.io/web-scraping-tools/ja3-fingerprint)
   — *live* tool: fingerprints **your own browser's** live TLS connection and
   reports JA3, JA3N and JA4. Server-side capture of the real handshake; no
   "paste a ClientHello" mode.
2. **suip.biz — Show TLS client fingerprint** — live scanner that reports JA3,
   JA3_FULL, JA3N, JA3N_FULL, JA4, JA4_R for the connecting client.
3. **Salesforce/ja3** (github.com/salesforce/ja3) — the reference Python/Zeek/
   Suricata implementation. Computes JA3 (client) and JA3S (server) from a pcap.
4. **Cloudflare / AWS WAF JA3/JA4** — platform features that expose the JA3/JA4
   of inbound requests as a rule field; not a calculator.
5. **ja3er.com / community DBs** — crowdsourced JA3 → known-client lookups.

## Gap analysis (fit-to-model)

| Competitor capability | In our model? | Action |
|---|---|---|
| JA3 string + MD5 from a ClientHello | yes (core) | shipped |
| GREASE removal (RFC 8701) | yes | shipped |
| Accept record/handshake/body hex, lenient separators | yes | shipped (better than most — they only take live traffic) |
| Show decoded ciphers / extensions / curves / SNI | yes | shipped (most live tools only print the hash) |
| **JA3N (normalized, sorted extensions)** | **yes — pure sort+MD5** | **ADDED this pass** — closes the main gap vs Scrapfly/suip, and is the more useful fingerprint for modern browsers that randomize extension order (Chrome 110+, Firefox 114+) |
| JA4 / JA4+ family | partially out-of-model | **NOT built** — JA4 needs a different format (protocol+ALPN+SNI-flag+cipher/ext counts+truncated SHA-256 of sorted lists+signature-algorithms). Computable in pure Rust in principle but it is a *distinct fingerprint scheme*, not a JA3 variant; out of scope for a "JA3" tool. Candidate for a separate `ja4-fingerprint` tool. |
| JA3S (server fingerprint from ServerHello) | in-model but out-of-scope | **NOT built** — JA3S fingerprints the **ServerHello**, a different input; belongs in a separate tool. |
| Live capture of the caller's own TLS handshake | out-of-model | **NOT built** — gizza tools take explicit input; there is no server-side TLS terminator to sniff the live handshake. Stated as a limitation. |
| JA3 → known-client database lookup | out-of-model | **NOT built** — needs an external/maintained dataset; gizza tools are offline/pure. |

## Changes made this pass

- Added **JA3N** (extensions sorted ascending) — `ja3n` + `ja3n_md5` — to the
  core, the chat skill/CLI output, and the page. New unit test
  `ja3n_sorts_extensions` proves JA3 ≠ JA3N when extensions are unsorted and that
  JA3N matches the canonical sorted form. CLI + Playwright verified.
- Documented JA3N in the descriptor, manifest, meta hero/description/tags and the
  page copy.

## Verification

- `cargo test --workspace`: 8 tests pass (1 schema drift-guard + 7 core).
- `wafer build`: block validates/instantiates (md5 is wasm-safe), 313 KiB.
- `wasm-pack` web build + generator render: OK.
- CLI: `gizza tool ja3-fingerprint client_hello=<hex>` → correct JA3/JA3N for
  both sorted- and unsorted-extension ClientHellos.
- Playwright `tool-page-ja3-fingerprint.spec.ts`: 2 tests pass (JA3 string/MD5/
  JA3N/version/SNI, and the 0x-prefix path).

## Out-of-model / not built (recorded honestly)

- JA4 / JA4+ (distinct scheme — separate tool).
- JA3S (ServerHello input — separate tool).
- Live TLS-handshake capture of the caller (no server-side terminator).
- JA3 → client-name database lookup (needs an external dataset).

No competitor copy, branding, or trademarks were reproduced.
