# cmac-generate — competitor analysis (2026-06-22)

Tool: **AES-CMAC Generator** (`blocks/cmac-generate`). Computes an AES-CMAC
(Cipher-based MAC, NIST SP 800-38B / RFC 4493) over a message with a 128/192/256-bit
AES key. Pure Rust (RustCrypto `cmac` + `aes`) → runs on chat + CLI + page.

## Competitors surveyed

| Tool | Ciphers | AES key sizes | Msg encodings | Key encodings | Output | Verify mode | Notes |
|---|---|---|---|---|---|---|---|
| lddgo.net CMAC Calculate | AES, DES, SM4 | 128/192/256 | string/hex/base64/binary | string/hex/base64/binary | hex/base64/binary | no | server-side; many charsets |
| jsontotable.org AES-CMAC Generator/Verifier | AES | 128/256 (no 192) | utf8/hex/base64 | hex | hex/base64 | yes (verify tag) | page was 404 at fetch time; per search snippet |
| 8gwifi.org | broad crypto suite | — | — | — | — | — | no dedicated CMAC page found; has HMAC + AES cipher tools |
| paymentcardtools / FINT AES calculators | AES block ops | 128/192/256 | hex | hex | hex | no | raw AES blocks, not CMAC chaining |
| artjomb cryptojs-extension | AES (CMAC lib) | any AES | — | — | — | n/a | JS library demo, not a finished tool |

## Capability diff vs. our tool

What we already match or beat:
- **All three AES key sizes (128/192/256)** selected automatically by key length — at
  least one popular competitor (jsontotable) only does 128/256. We cover the full set.
- **Three message encodings** (text / hex / base64) and **three key encodings**
  (text / hex / base64) — matches lddgo's breadth; jsontotable forces a hex key only,
  we additionally allow text/base64 keys.
- **Two output formats** (hex + base64) plus an **uppercase-hex** toggle (most
  competitors emit lowercase hex only).
- **Standards-correct**: validated against the RFC 4493 / NIST SP 800-38B published
  vectors (AES-128 empty / 16 / 40 / 64-byte messages, AES-192 + AES-256 empty &
  16-byte) as unit tests, so output is provably correct — competitors show no vectors.
- **Privacy**: runs entirely client-side (WASM); lddgo and jsontotable are server-side
  posts. This is our core differentiator for secret keys.
- **Clear key-length error** with remediation ("set key_encoding to hex/base64"),
  which the surveyed tools don't surface helpfully.

In-model gaps considered and closed / declined:
- **Binary I/O format** (lddgo offers binary in/out): declined — a binary tag/key isn't
  representable in a text page field or a JSON chat arg; hex/base64 fully cover binary
  data losslessly, so this is a non-gap for our surfaces.
- **Verify mode** (jsontotable lets you check an expected tag): this is constant-time
  compare of a recomputed tag, which the user can do by generating and comparing; a
  dedicated verify surface would be a separate boolean-output tool. Noted as a possible
  future tool, not built here (keeps this tool single-purpose like hmac-generate).
- **Other block ciphers** (DES/SM4-CMAC via lddgo): out of scope — DES is deprecated and
  SM4 is niche; AES-CMAC is the universally-deployed variant (RFC 4493, IEEE 802.1X).
  Could be a future enhancement but adds little value and DES is actively discouraged.
- **Charset selection** (lddgo's ASCII/UTF-16/GBK/…): declined — UTF-8 is the web
  standard; exotic charsets are an anti-feature for a modern tool and any byte sequence
  can be supplied via hex/base64.

## Result

The tool meets or exceeds the surveyed competitors on key-size coverage, encoding
flexibility, output options, standards-correctness, and privacy. No in-model capability,
copy, or UX gap remained open. No competitor copy/branding/trademarks were used.

## Verification (this run)

- `cargo test --workspace` in `blocks/cmac-generate` — 17 core vectors + drift-guard schema test pass.
- `wafer build` — chat `block.wasm` instantiates + validates (347.6 KiB).
- `wasm-pack build` — page wasm built; generator rendered `pkg/tools/cmac-generate/`.
- CLI: `gizza tool cmac-generate …` returns RFC 4493 AES-128 tag `070a16b46b4d4144f79bdd9dd04a287c`, empty-message tag `bb1d6929e95937287fa37d129b756746`, AES-256 base64, and the key-length error (exit 1).
- Playwright `tool-page-cmac-generate.spec.ts` — 6/6 pass (hex compute, empty msg, base64, uppercase, error, query-param deep-link).
