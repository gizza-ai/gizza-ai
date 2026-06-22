# text-encrypt — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/text-encrypt` — encrypt or decrypt text with a passphrase
using AES-256-GCM. Pure-Rust (reuses the `encrypt-file` crypto core). Pure-text
input → text output, so chat + CLI + a page.

## Relationship to `encrypt-file`

This is the **text-native sibling** of `encrypt-file`: same AES-256-GCM /
PBKDF2-HMAC-SHA256 scheme and the same self-describing blob, but it takes a text
string inline (not a file `url`/`ref`) and returns a compact **base64 token**
instead of a binary file. Use `text-encrypt` to lock a short message/snippet you
paste into chat or a page; use `encrypt-file` for whole files. (The earlier
file-based duplicates `decrypt-file` / `file-encryptor` were skiplisted; this is a
genuinely different I/O modality, not a dup.)

## What competitors do

- **Online "encrypt text" sites** — paste text + password, get ciphertext.
  **Weakness: you paste the secret + password into a third-party page.**
- **`openssl enc -aes-256-gcm` / `gpg -c`** — local + strong, but fiddly CLI
  (openssl GCM CLI is notoriously awkward; salt/iter flags), and not browser/chat
  runnable.
- **Browser crypto demos** — vary wildly in quality; many use weak KDFs (raw
  password as key, no salt) or AES-CBC without authentication.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat Service
   Worker, CLI, and in-browser page. Text + passphrase never leave the device.
2. **Authenticated, salted, stretched.** AES-256-**GCM** (tamper-evident) with a
   key from **PBKDF2-HMAC-SHA256, 200k iterations**, and a **fresh random salt +
   nonce per encryption** (so the same text encrypts to different tokens). Many
   quick web tools get one of these wrong.
3. **Copy-pasteable token.** Output is compact base64 you can drop into an email,
   note, or chat — and decrypt anywhere this tool runs.
4. **Fails safe.** Wrong passphrase or a single altered byte → clean error (the
   GCM tag won't verify), never silent garbage.
5. **Same everywhere + bidirectional.** One tool, `mode=encrypt|decrypt`, identical
   via chat, CLI, and a `?text=…&passphrase=…&mode=…` page.

## Honest scope

- **Confidentiality with a shared passphrase** — symmetric, not public-key; both
  sides need the passphrase (out-of-band). For recipient public keys, see the PGP
  tools.
- Security is only as strong as the passphrase; the tool can't enforce strength.

## Tests

4 core unit tests: a full **round-trip** (incl. a multi-byte emoji) where the
token is base64 and not the plaintext; **wrong passphrase fails**; encryption is
**non-deterministic** (two tokens differ); and error cases (empty passphrase,
non-base64, valid-base64-but-bad-header). Plus the block drift-guard schema test.
**CLI verified** end-to-end (encrypt → token → decrypt → original). **Page**
verified with Playwright (encrypt then decrypt round-trip in the browser). `wafer
build` instantiates the chat block.
