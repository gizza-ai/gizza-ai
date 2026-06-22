# rsa-sign — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/rsa-sign` — sign a message with an RSA private key using
PKCS#1 v1.5 or PSS (SHA-256/384/512), returning a base64 signature. Pure-Rust
(`rsa` + `sha2`). Text input → text output: chat + CLI + a page.

## What competitors do

- **`openssl dgst -sign` / `openssl pkeyutl -sign`** — the reference, local and
  correct, but the CLI is famously fiddly (key formats, `-sigopt rsa_padding_mode`,
  piping the digest) and isn't browser-runnable.
- **Online "RSA sign" tools** — paste a key + message, get a signature. **Major
  weakness: you paste your *private key* into a third-party web page.**
- **Language libraries** (`cryptography` in Python, `node:crypto`, JOSE libs) —
  correct but require writing/running code and choosing padding/hash correctly.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`rsa`) compiled to wasm: runs
   in the chat Service Worker, the CLI, and in-browser on the page. The private
   key never leaves the device — essential for a signing key.
2. **Both schemes, three hashes, one tool.** `pkcs1v15` (deterministic, classic)
   and `pss` (randomized, modern) × SHA-256/384/512 — selectable without
   remembering OpenSSL's `-sigopt` incantations.
3. **Flexible key input.** Accepts PEM in PKCS#8 *or* PKCS#1 form, auto-detected.
4. **Standard, verifiable output.** Raw signature, base64-encoded; verifies with
   any standard RSA library using the same scheme + hash and the matching public
   key.
5. **Same everywhere.** Identical behaviour via chat, CLI (`gizza tool rsa-sign
   …`), and a `?message=…&private_key=…&scheme=…&hash=…` page.

## Honest scope

- **Signs the message** (hashing it with the chosen algorithm); signing a
  pre-computed digest (`sign_prehash`) is not exposed.
- **RSA only** — Ed25519/ECDSA signing are out of scope here.
- **No verification** in this tool (it produces signatures; verify with the public
  key in your own library).

## Tests

4 core unit tests using a freshly generated RSA-2048 key: a PKCS#1 v1.5 / SHA-256
signature **verifies** with the public key and **fails on a tampered message**; a
PSS / SHA-512 signature verifies; PKCS#1 v1.5 is confirmed **deterministic** (same
input → identical signature); and parsing/erroring on a bad key, bad scheme, and
bad hash. Plus the block drift-guard schema test. **CLI verified** end-to-end
(sign with an OpenSSL-generated key, signature re-verified). **Page** verified with
Playwright (multiline message + key, scheme/hash selects → a base64 signature).
`wafer build` instantiates the chat block.
