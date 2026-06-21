# aes-cipher — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/aes-cipher` — encrypt/decrypt text with AES in CBC, CTR, GCM, or
ECB across 128/192/256-bit keys, with hex/base64 I/O. Pure-Rust (RustCrypto:
`aes`, `cbc`, `ctr`, `ecb`, `aes-gcm`). Pure-text input → text output: chat + CLI +
a page.

## Relationship to the other crypto tools

`text-encrypt` / `encrypt-file` are the **safe, high-level** path: a passphrase,
PBKDF2 key derivation, and a random salt + nonce handled for you (always AEAD).
`aes-cipher` is the **low-level developer** tool: you bring the **raw key, IV and
mode**. Different audience (implementing/debugging a spec, interop testing,
learning), so it complements rather than duplicates them.

## What competitors do

- **Online AES tools** (cyberchef recipes aside, many "aes encryption online"
  sites) — flexible, but you **paste the key + data into a third-party page**, and
  mode/padding handling is often unclear or wrong.
- **`openssl enc -aes-256-cbc …`** — local and canonical, but the CLI is famously
  awkward (key/iv hex flags, GCM barely supported via `enc`), and not browser/chat
  runnable.
- **CyberChef** — excellent and local-in-browser, but a whole separate app to load
  and wire up a recipe.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat Service
   Worker, CLI, and in-browser page. Key and data never leave the device.
2. **All four modes, all three key sizes, both directions** in one call, with the
   key size inferred from the key length — no separate "AES-256" switch to get
   wrong.
3. **Correct, library-grade primitives.** RustCrypto's `cbc`/`ctr`/`ecb`/`aes-gcm`
   with PKCS#7 padding for block modes; GCM is authenticated (the 16-byte tag is
   appended and verified on decrypt, so tampering fails cleanly).
4. **hex or base64** for key/IV/ciphertext via one `format` switch; clear errors
   for wrong key/IV lengths or bad encodings.
5. **Honest safety guidance.** ECB is labelled insecure, and the description points
   passphrase users to `text-encrypt` instead.

## Honest scope

- **Text in/out** (UTF-8 plaintext; ciphertext hex/base64). For whole **files**,
  use `encrypt-file`. (One block can't cleanly take both a text field and a file
  upload.)
- **Raw key/IV** — no key derivation; you must supply correctly-sized key and IV.
  No AAD input for GCM.

## Tests

7 core unit tests: round-trip for **CBC (128/192/256)**, **CTR**, **GCM
(128/192/256)**, and **ECB**; GCM **detects tampering** (flipped byte → error); a
wrong CBC key fails (PKCS#7 unpad); and parameter errors (bad key length, missing
IV, bad mode/format). Plus the block drift-guard schema test. **CLI verified**
end-to-end (encrypt → decrypt round trip; cross-checked with OpenSSL where
applicable). **Page** verified with Playwright. `wafer build` instantiates the chat
block.
