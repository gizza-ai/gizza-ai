# gost-kuznyechik-cipher — competitor analysis (2026-06-23)

Tool: encrypt/decrypt text with the GOST R 34.12-2015 "Kuznyechik" 128-bit block
cipher (RFC 7801), low-level (user supplies raw key/IV/mode). Pure-Rust
(RustCrypto `kuznyechik` 0.8 + `cbc`/`ctr`/`cfb-mode`/`ofb`/`ecb`). Runs on all
surfaces (chat, CLI, browser page).

## Surfaces verified (Phase 1)

- **Chat block**: `wafer build` validates `target/block.wasm` instantiates (337 KiB).
- **CLI**: `gizza tool gost-kuznyechik-cipher …` round-trips encrypt→decrypt in
  CBC and CFB (hex), correct output.
- **Page**: `tests/tool-page-gost-kuznyechik-cipher.spec.ts` — CBC encrypt→decrypt
  round-trip + bad-key-length error. Both pass (Playwright/chromium).
- **Correctness**: a unit test checks a single-block ECB output against the
  **RFC 7801 / GOST R 34.13-2015 Appendix A.1** test vector
  (`1122334455667700ffeeddccbbaa9988` → `7f679d90bebc24305a468d42b9d4edcd`).

## Competitors surveyed (top 5)

1. **gostcrypt** (github.com/pedroalbanese/gostcrypt) — the dedicated Kuznyechik
   CLI. Offers MGM (authenticated) mode, raw 256-bit key (`-k`), and PBKDF2
   password-based encryption (`-p`), decrypt flag (`-d`).
2. **codertools AES tool** (codertools.net/tools/aes.php) — general block-cipher
   web tool: CBC/ECB/CTR/OFB/CFB modes, hex/base64/raw key+IV, 100% client-side.
3. **toolshu AES** (toolshu.com) — CBC/ECB/CFB/CTR/OFB, base64 + hex output.
4. **devglan AES** (devglan.com/online-tools/aes-encryption-decryption) —
   ECB/CBC/CTR/GCM, key-size selectable.
5. **cryptii** (cryptii.com) — modular in-browser encode/encrypt pipeline; AES
   symmetric; no Kuznyechik.

There is **no prominent dedicated browser Kuznyechik encrypt/decrypt tool** — the
only Kuznyechik-specific competitor is the gostcrypt CLI. The web tools above are
all AES-only and don't implement Kuznyechik at all, so gizza's in-browser
Kuznyechik page is differentiated.

## Gap diff + ranking (fit-to-model)

| Gap | In model? | Action |
|---|---|---|
| CFB mode | yes (`cfb-mode` 0.8 on cipher 0.4) | **CLOSED** — added |
| OFB mode | yes (`ofb` 0.6 on cipher 0.4) | **CLOSED** — added |
| CBC / CTR / ECB | already shipped | kept |
| hex + base64 I/O for key/iv/ct | already shipped | kept |
| RFC 7801 test-vector correctness | matched | **added unit test** |
| MGM authenticated mode | no | out-of-model — no maintained RustCrypto Kuznyechik-MGM wrapper; documented as N/A |
| PBKDF2 / password-based encryption | no (different model) | out-of-model — belongs to the passphrase tool; page points users to `text-encrypt` for safe salt/KDF/nonce |
| GCM | no | Kuznyechik isn't paired with GCM in RustCrypto; not standard for this cipher |

## Changes made this run

- Added **CFB** and **OFB** stream modes (matches the mode breadth of the general
  cipher web tools), with round-trip unit tests for both.
- Mode set is now `cbc` (default) / `ctr` / `cfb` / `ofb` / `ecb`, reflected in the
  descriptor enum, authored drift schema, manifest, page `<select>`, copy and tags.
- Added an RFC 7801 single-block ECB correctness test vector.
- Wrote SEO copy + privacy/"which tool" guidance pointing passphrase users to
  `text-encrypt`.

No competitor copy, branding, or trademarks were copied.
