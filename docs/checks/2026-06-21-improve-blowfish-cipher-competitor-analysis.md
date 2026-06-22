# blowfish-cipher — competitor analysis (2026-06-21)

Tool: `gizza tool blowfish-cipher` / `/tools/blowfish-cipher/` — encrypt or decrypt
data with the Blowfish block cipher in ECB or CBC mode, variable 4–56 byte key,
8-byte IV (CBC), PKCS#7 padding, hex/base64 I/O. Pure-Rust (RustCrypto
`blowfish` 0.9 + `cbc`/`ecb`), runs on all three surfaces (chat, CLI, browser
page). Sibling of `des-cipher` / `aes-cipher`.

## Surface check (Phase 1)

- **Chat (block.wasm):** `wafer build` validated `target/block.wasm` (326.8 KiB);
  drift-guard unit test `schema_json_matches_authored_chat_schema` passes.
- **CLI:** `gizza tool blowfish-cipher …` — CBC encrypt→decrypt round-trip OK,
  ECB+base64 OK, short-key error surfaced correctly.
- **Page:** Playwright `tool-page-blowfish-cipher.spec.ts` — CBC encrypt then
  decrypt round-trip in-browser passes (709 ms).

## Competitors surveyed

1. **sladex.org blowfish.js** — modes ECB/CBC/PCBC/CFB/OFB/CTR; output
   Base64/Hex/String/Raw.
2. **lddgo.net Blowfish** — modes CBC/CFB/OFB/CTR/ECB; formats hex/string/base64.
3. **codertools.net Blowfish** — ECB & CBC with PKCS7 padding; key input as
   UTF-8 / hex / base64.
4. **Boxentriq Blowfish Cipher** — 64-bit block, variable key up to 448 bits;
   ECB/CBC/CFB/OFB/CTR.
5. **blowfish.online-domain-tools.com** — string/file encrypt-decrypt, ECB and
   other modes.

## Gap diff (fit-to-model)

| Capability | Competitors | gizza blowfish-cipher | Status |
|---|---|---|---|
| ECB mode | yes | yes | parity |
| CBC mode + IV | yes | yes | parity |
| PKCS#7 padding | yes | yes (CBC + ECB) | parity |
| Variable key 4–56 B | yes | yes (validated, with min/max error) | parity |
| Hex key/iv/ciphertext | yes | yes | parity |
| Base64 key/iv/ciphertext | yes | yes (default) | parity |
| UTF-8 plaintext (unicode) | yes | yes (round-trips emoji) | parity |
| Browser-local / no upload | mixed | yes (all compute in-browser wasm) | **advantage** |
| Chat + CLI surfaces | no (web only) | yes | **advantage** |
| Known-answer correctness | — | Schneier ECB vector asserted in unit test | **advantage** |

### In-model gaps NOT yet built (documented, not shipped)

- **Stream/feedback modes CFB / OFB / CTR.** RustCrypto ships `cfb-mode` /
  `ofb` / `ctr` over the same `blowfish::Blowfish`, so these are in-model. They
  were deliberately deferred to keep parity with the established `des-cipher`
  ECB/CBC template and stay inside the build/honesty gate; CBC+ECB cover the
  dominant interop cases. A future pass can add a `cfb|ofb|ctr` option to the
  `cipher` enum across core + descriptor + web.
- **UTF-8 / raw key input.** Some competitors accept a key typed as plain text.
  gizza takes the key encoded (hex/base64) via the single `format` selector,
  which is unambiguous; a separate key-encoding selector would be the way to add
  raw-text keys without overloading `format`. Deferred (low value vs. ambiguity).
- **File encryption.** Out of this text-cipher tool's model — `encrypt-file`
  (AES-GCM) is gizza's file-encryption tool.

## Conclusion

Ships at parity with the common ECB/CBC + hex/base64 Blowfish web tools, plus
gizza's structural advantages (in-browser wasm, chat + CLI, asserted KAT
correctness). The only genuine in-model gap is the additional feedback modes
(CFB/OFB/CTR), logged above for a follow-up. No competitor copy, branding, or
trademarks were used.
