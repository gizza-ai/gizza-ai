# gost-magma-cipher — competitor analysis (2026-06-23)

Tool: encrypt/decrypt text with the GOST 28147-89 / GOST R 34.12-2015 "Magma"
64-bit block cipher (RFC 8891), low-level (user supplies raw key/IV/mode).
Pure-Rust (RustCrypto `magma` 0.9 + `cbc`/`ecb` 0.1 on cipher 0.4). Runs on all
surfaces (chat, CLI, browser page).

## Surfaces verified (Phase 1)

- **Chat block**: `wafer build` validates `target/block.wasm` instantiates (328 KiB).
- **CLI**: `gizza tool gost-magma-cipher …` round-trips encrypt→decrypt in CBC and
  ECB (hex), correct output; bad key length returns a clean error message.
- **Page**: `tests/tool-page-gost-magma-cipher.spec.ts` — CBC encrypt→decrypt
  round-trip + bad-key-length error. Both pass (Playwright/chromium).
- **Correctness**: a unit test checks a single-block ECB output against the
  **GOST R 34.12-2015 / RFC 8891** Magma test vector (key
  `ffeeddcc…fcfdfeff`, plaintext block `fedcba9876543210` →
  ciphertext `4ee901e5c2d8ca3d`).

## Competitors surveyed (top 5)

1. **gostcrypt / gostfish** (github.com/pedroalbanese) — dedicated GOST 28147-89
   CLI tooling: raw 256-bit key, ECB/CBC/CFB/CTR (CNT) modes, MAC/IMIT, password
   derivation. The closest dedicated Magma implementation.
2. **CyberChef** (gchq.github.io/CyberChef) — has "GOST Encrypt"/"GOST Decrypt"
   operations (Magma / 28147-89) with selectable S-box set, block mode (ECB/CFB/
   OFB/CTR/CBC), key/IV as hex; in-browser pipeline.
3. **codertools / toolshu / devglan AES web tools** — general block-cipher web
   tools (CBC/ECB/CTR/CFB/OFB, hex/base64 key+IV, client-side) but **AES-only**,
   no Magma.
4. **cryptii** (cryptii.com) — modular in-browser encrypt pipeline; symmetric AES;
   no Magma.
5. **online GOST calculators** (various RU sites) — mostly hashing (Streebog /
   GOST R 34.11) rather than Magma block encrypt/decrypt; several are server-side.

There is **no prominent dedicated single-purpose browser Magma encrypt/decrypt
tool**; CyberChef's GOST op is the only mainstream in-browser Magma, and it is one
operation inside a large recipe builder. gizza's standalone in-browser Magma page
(plus CLI and chat) is differentiated.

## Gap diff + ranking (fit-to-model)

| Gap | In model? | Action |
|---|---|---|
| ECB + CBC modes | yes (`ecb`/`cbc` 0.1 on cipher 0.4) | shipped |
| hex + base64 I/O for key/iv/ct | yes | shipped |
| GOST R 34.12-2015 / RFC 8891 test-vector correctness | matched | **added unit test** |
| Selectable S-box set (e.g. CryptoPro vs tc26-Z) | no (RustCrypto `magma` only ships the standardized `id-tc26-gost-28147-param-Z` S-box; older CryptoPro/test S-boxes are not exposed) | out-of-model — documented that the standard tc26-Z S-box is used |
| CFB / OFB / CTR (CNT) stream modes | partially (generic crates exist) | scoped to ECB/CBC for this release to match the standardized low-level presentation; can be added later like the Kuznyechik tool |
| MAC / IMIT (GOST 28147 MAC) | no | out-of-model — no maintained RustCrypto Magma-MAC wrapper |
| PBKDF2 / password-based encryption | no (different model) | out-of-model — page points users to `text-encrypt` for safe salt/KDF/nonce |

## Changes made this run

- Built the tool end-to-end: ECB (default-less) + CBC (default) modes, 256-bit key,
  8-byte IV, hex/base64 I/O, PKCS7 padding; descriptor/manifest/page kept in sync
  with a chat-schema drift-guard test.
- Added a **GOST R 34.12-2015 / RFC 8891** single-block correctness test vector so
  the implementation is verified against the published standard, not just round-trip.
- Pinned `magma = "0.9"` (cipher 0.4) so it composes with the `cbc`/`ecb` 0.1 mode
  crates and instantiates in the wafer (wasm32-wasip1) chat runtime; magma 0.10 is
  on cipher 0.5 and does not compose with the 0.1 mode crates.
- Wrote SEO copy + privacy/"which tool" guidance pointing passphrase users to
  `text-encrypt` and 128-bit users to `gost-kuznyechik-cipher`.

No competitor copy, branding, or trademarks were copied.
