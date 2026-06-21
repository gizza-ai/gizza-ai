# des-cipher — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/des-cipher` — encrypt or decrypt data with single DES in ECB or
CBC mode, hex/base64 I/O. Pure-Rust (RustCrypto `des` + `cbc`/`ecb`). Pure-text
input → text output: chat + CLI + a page. A **legacy/interop** companion to
`aes-cipher`.

## What competitors do

- **`openssl enc -des-cbc …`** — local + canonical, but DES support is buried in
  legacy flags and the CLI is fiddly; not browser-runnable.
- **Online DES tools** — exist for legacy interop, but you **paste keys + data into
  a third-party page**, and quality/escaping varies.
- **Java/old enterprise libraries** — where most DES data originates; this tool
  lets you decrypt that data without standing up a JVM.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat, CLI, and
   an in-browser page. Keys and data never leave the device.
2. **Decrypt legacy data without the legacy stack.** The honest use case for DES is
   interop / decrypting old payloads; this gives you ECB + CBC with PKCS#7 padding
   and hex/base64 in one call.
3. **Correct primitives.** RustCrypto `des` with `cbc`/`ecb`; clear errors for the
   wrong key/IV length (DES requires exactly 8 bytes each) or bad encodings.
4. **Loudly honest about security.** The description and page warn that DES is a
   broken 56-bit cipher and point users to `aes-cipher` / `text-encrypt` for real
   encryption — rather than silently offering insecure crypto.

## Honest scope

- **Single DES, ECB/CBC** — not 3DES/DESede, and intentionally no stream/AEAD
  modes. **Insecure by design**; provided for interop and legacy decryption only.
- Text I/O (UTF-8 plaintext; ciphertext hex/base64).

## Tests

4 core unit tests: CBC round-trip (incl. a multi-byte emoji); ECB round-trip with a
base64 key; wrong-key failure; and parameter errors (key not 8 bytes, missing IV,
bad mode/format). Plus the block drift-guard schema test. **CLI verified** end-to-
end (CBC encrypt→decrypt). **Page** verified with Playwright (browser round-trip).
`wafer build` instantiates the chat block (321 KiB).
