# ja4-server-fingerprint — competitor analysis (2026-06-23)

Tool: **JA4S Fingerprint Calculator** — paste a TLS ServerHello as hex, get the
JA4S server fingerprint string (+ the raw `JA4S_r` variant and decoded fields).
Pure-Rust, runs on all backends (chat / CLI / page).

## What the tool does

Parses a ServerHello (optionally prefixed by the TLS record header `16 03 ...`
and/or the handshake header `02 ...`) and builds the **JA4S** fingerprint per the
FoxIO JA4+ specification:

```
(t|q)(version)(extcount)(alpn) _ (cipher) _ (sha256_of_extensions)[:12]
```

- transport `t`=TCP / `q`=QUIC (caller-supplied, since it isn't in the bytes);
- TLS version 2-char code (`13/12/11/10/s3/s2`), from the `supported_versions`
  extension if present, else `legacy_version`;
- extension count (2 digits, capped 99);
- ALPN = first+last char of the chosen protocol (`00` none, `99` non-ASCII);
- the single chosen cipher suite (4 hex chars);
- first 12 hex chars of SHA256 of the comma-joined extension-type list, in wire
  order, **GREASE kept** (`000000000000` if no extensions).

Returns: `ja4s`, `ja4s_r` (extension list un-hashed), `transport`,
`tls_version`, `cipher`, `extensions[]`, `alpn`.

## Top competitors / references surveyed

1. **FoxIO-LLC/ja4** (github.com/FoxIO-LLC/ja4) — the canonical JA4+ reference
   implementation (Python/Rust/Zeek/Wireshark). `to_ja4s()` in `python/ja4.py`
   defines the exact algorithm this tool implements (format `a_b_c`, ext hash =
   `sha256(','.join(exts))[:12]`, version from `supported_versions`, ALPN
   first+last). It works from a **pcap**, not a paste-in-hex calculator.
2. **Scrapfly — JA3/JA4 TLS Fingerprint tool** — *live* tool that fingerprints
   the connecting browser's own TLS handshake and reports JA3/JA3N/JA4. Client
   side only; no server-fingerprint-from-bytes mode.
3. **suip.biz / browserleaks TLS** — live client TLS fingerprint scanners
   (JA3/JA4/JA4_R). No ServerHello → JA4S calculator.
4. **Cloudflare / Akamai / AWS bot-management** — platforms that expose JA4/JA4S
   of traffic as a rule field for detection; not a public calculator.
5. **Wireshark JA4+ plugin** — decodes JA4S inline while inspecting a capture;
   requires the full pcap and the plugin, not a quick paste tool.

## Gap analysis (fit-to-model)

| Competitor capability | In our model? | Action |
|---|---|---|
| JA4S string from a ServerHello | yes (core) | shipped |
| Exact FoxIO algorithm (version from supported_versions, ext SHA256[:12], ALPN first+last, GREASE kept, wire order) | yes | shipped — verified against the `to_ja4s()` reference and hand-computed vectors |
| TCP vs QUIC transport prefix | yes | shipped (a `quic` boolean — the one field not derivable from the bytes) |
| Accept record/handshake/body hex, lenient separators | yes | shipped (better than the pcap-only references) |
| Show decoded version / cipher / extensions / ALPN | yes | shipped (live tools usually print only the hash) |
| Raw `JA4S_r` (extension list un-hashed) | yes | shipped (matches the reference's `_r` output, useful for debugging) |
| JA4 (client), JA4H (HTTP), JA4X (cert), JA4SSH, JA4L | out-of-scope here | **NOT built** — each is a distinct JA4+ scheme with its own input. Candidates for separate tools (a `ja4-fingerprint` client tool would pair naturally with this). |
| Live capture of a server's handshake from a hostname | out-of-model | **NOT built** — gizza tools take explicit input; there is no outbound TLS client to dial a host and sniff its ServerHello. Stated as a limitation. |
| JA4S → known-server / threat-intel database lookup | out-of-model | **NOT built** — needs an external maintained dataset; gizza tools are offline/pure. |

## Why this is not a duplicate of `ja3-fingerprint`

`ja3-fingerprint` fingerprints the **ClientHello** with the JA3 scheme
(comma-joined decimal fields → MD5). JA4S fingerprints the **ServerHello** with
the JA4+ scheme (transport/version/ext-count/ALPN + cipher + truncated SHA256).
Different input message, different algorithm, different output format. The
ja3-fingerprint analysis doc itself flagged a server-side JA4 tool as a separate
candidate.

## Verification

- `cargo test --workspace`: 10 tests pass (1 schema drift-guard + 9 core,
  including TLS 1.3+ALPN, TLS 1.2 no-extensions, QUIC prefix, ALPN-none, garbage
  rejection, separator/0x handling).
- `wafer build`: block validates/instantiates (sha2 is wasm-safe), 311 KiB.
- `wasm-pack` web build + generator render (246 tools): OK.
- CLI: `gizza tool ja4-server-fingerprint server_hello=<hex>` → `t1303h2_c02b_19fd10492780`;
  with `quic=true` → `q1303h2_c02b_19fd10492780`.
- Playwright `tool-page-ja4-server-fingerprint.spec.ts`: 2 tests pass (TCP JA4S
  string + decoded fields, and the QUIC checkbox path → `q` prefix).

## Out-of-model / not built (recorded honestly)

- Other JA4+ members (JA4 client, JA4H, JA4X, JA4SSH, JA4L) — distinct schemes,
  separate tools.
- Live ServerHello capture by dialing a hostname (no outbound TLS client).
- JA4S → server/threat-intel database lookup (needs an external dataset).

No competitor copy, branding, or trademarks were reproduced; the algorithm is
the open FoxIO JA4S specification.
