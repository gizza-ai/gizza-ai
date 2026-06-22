# hmac-generate — competitor analysis (2026-06-22)

## Surfaces verified

- **Chat block** — `wafer build` validates + instantiates `target/block.wasm`
  (375 KiB); schema single-sourced from `descriptor()`, drift guard passes.
- **CLI** — `gizza tool hmac-generate message=… key=…` returns the correct tag
  for sha256 (default), md5, base64 output, and hex-encoded key; invalid
  algorithm exits non-zero with a clear message.
- **Page** — `/tools/hmac-generate/`, 7 Playwright tests pass (default sha256,
  md5, sha3-512, base64 output, uppercase checkbox, hex key-encoding,
  query-param deep-link).

All outputs cross-checked against Python `hmac`/`hashlib` and the RFC 2202 /
RFC 4231 published test vectors.

## Top competitors surveyed

1. **freeformatter.com — HMAC Generator** — message + secret key, algorithm
   dropdown (MD5, SHA-1, SHA-224/256/384/512, SHA3 family, RIPEMD), hex output.
2. **devglan.com — Online HMAC Generator** — message + key, algorithm select,
   hex output, copy button.
3. **codebeautify.org — HMAC Generator** — message + key + algorithm, output as
   a single hex string.
4. **dencode.com / 8gwifi.org HMAC tools** — message + key, algorithm select,
   and notably **selectable key/message input encoding** (text/hex/base64) and
   **selectable output encoding** (hex/base64).
5. **CyberChef "HMAC" operation** — key with a key-type selector
   (UTF-8/hex/base64/latin1) and a hashing-function dropdown; output is raw
   bytes rendered by downstream operations.

## Capability diff & gaps closed

| Capability | Competitors | gizza hmac-generate |
|---|---|---|
| Selectable hash (MD5/SHA-1/SHA-2/SHA-3) | most | ✅ 8 algorithms (md5, sha1, sha224/256/384/512, sha3-256/512) |
| HMAC-SHA256 default | common | ✅ default `sha256` |
| Key input encoding (text/hex/base64) | only 8gwifi/dencode/CyberChef | ✅ `key_encoding` |
| Message input encoding (text/hex/base64) | rare | ✅ `message_encoding` |
| Output as hex **and** base64 | only dencode/CyberChef | ✅ `output_format` |
| Uppercase hex toggle | rare | ✅ `uppercase` |
| Runs fully client-side / no upload | mixed (many POST to a server) | ✅ WASM in-browser, nothing uploaded |
| Deep-link / query-param prefill | none | ✅ `?message=…&key=…&algorithm=…` |

**Gaps intentionally closed** vs the median competitor (which is text-key +
hex-out only): binary key support via `key_encoding`, base64 output, and an
uppercase toggle — matching the most capable tools (dencode, CyberChef) while
keeping a single static page with no server round-trip.

## Out-of-model / not built (documented, not gaps in fit)

- **RIPEMD-160 / Whirlpool** HMAC variants (seen on freeformatter) — niche
  legacy hashes; the RustCrypto `ripemd` crate could be added later but is low
  value and omitted to keep the algorithm list focused on the widely-used set.
- **Verify mode** (compare a candidate tag with constant-time equality) — the
  tool generates tags; verification is a trivial string compare the caller can
  do. Documented in the page copy ("recompute and compare").
- **File input** — HMAC of an uploaded file would need an `AssetKind` file input
  on the page; the tool targets message/key strings, consistent with all
  surveyed competitors.

No competitor copy, branding, or trademarks were reproduced.
