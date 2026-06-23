# evp-derive — competitor analysis & improvement snapshot (2026-06-23)

**Tool:** `evp-derive` — reproduce OpenSSL's legacy `EVP_BytesToKey` key/IV
derivation from a password, salt, and hash. Pure-Rust, all backends (chat + CLI +
page). Verified against OpenSSL 3.5.6.

## What it does

Implements the OpenSSL `EVP_BytesToKey` KDF exactly as documented:

```
D_i = HASH^count(D_{i-1} ‖ password ‖ salt),  D_0 = empty
key ‖ iv = D_1 ‖ D_2 ‖ …   (first key_length bytes = key, next iv_length = IV)
```

Inputs: `password` (required), `salt` (+ `salt_encoding` utf8/hex/base64), `hash`
(md5/sha1/sha256/sha512), `key_length` (default 32), `iv_length` (default 16),
`count` (default 1), `encoding` (hex/base64). Output: derived key and IV.

## Competitor landscape (top references)

No widely-used *dedicated* "EVP_BytesToKey online" tool dominates search; the field
is reference docs + library code + a couple of generic crypto multitools:

1. **OpenSSL itself** (`openssl enc -P -pass pass:… [-md] [-S] [-iter] [-nosalt]`) —
   the authoritative reference. Prints `key=`/`iv=`/`salt=`. This is the ground
   truth our output is byte-for-byte verified against.
2. **CyberChef** — has a "Derive EVP key" operation: password, key size (bits), IV
   size (bits), salt, hash function (MD5/SHA1/SHA256/SHA384/SHA512). Part of a huge
   multitool; not a focused page, no per-vector docs.
3. **anothermh/evp_bytes_to_key** (Ruby gem) and many language snippets (Node
   `EVP_BytesToKey` ports, Python, Go) — library implementations, not user tools.
4. **forge / crypto-js docs** — CryptoJS's default `AES.encrypt(text, passphrase)`
   uses this MD5-based derivation; developers reverse-engineer it constantly.
5. **OpenSSL `EVP_BytesToKey(3)` man page** — the algorithm spec.

## Feature diff & gaps (fit-to-model)

| Capability | Competitors | evp-derive | Status |
|---|---|---|---|
| MD5 / SHA-1 / SHA-256 / SHA-512 digest | CyberChef (+SHA-384) | yes (4) | parity (SHA-384 niche, skipped) |
| Configurable key length | yes | yes (bytes) | parity |
| Configurable IV length | CyberChef (bits) | yes (bytes, 0 = none) | parity |
| Salt (utf8/hex/base64) | partial | yes, 3 encodings | **ahead** |
| Iteration count (`-iter`) | OpenSSL only | yes | **ahead of CyberChef** |
| hex / base64 output | hex | yes (both) | **ahead** |
| Byte-for-byte OpenSSL parity, documented vectors | implicit | yes (3 vectors in tests) | **ahead** |
| Runs locally, password never uploaded | varies | yes (wasm) | parity / privacy win |
| 3 surfaces (chat LLM API, CLI, page) | none | yes | **ahead** (unique to gizza) |

**Copy/UX additions made:** the page explains the algorithm, maps key/IV lengths to
AES-128/192/256 + DES, explains OpenSSL's 8-byte `-S` salt and the `Salted__` magic,
and the CryptoJS interop use-case. A prominent security note flags that this is a
**legacy** KDF (count=1 ≈ a single hash) and points to PBKDF2/scrypt/Argon2id for new
designs.

## In-model gaps closed

- Units chosen as **bytes** (matches OpenSSL's mental model and our sibling KDF tools
  `pbkdf2-derive`/`scrypt-derive`), with the AES-size mapping spelled out in copy so
  users coming from CyberChef's "bits" UI aren't confused.
- Both hex and base64 output (CyberChef is hex-only).
- Iteration `count` exposed (CyberChef omits it).
- Salt accepted as utf8 **and** raw hex/base64 — important because OpenSSL's salt is
  raw bytes, not text.

## Out-of-model / deliberately not built

- **Decrypting the ciphertext** end-to-end (the tool derives key+IV; actual
  AES-CBC decryption is a separate cipher tool / out of this tool's scope).
- **SHA-384** digest — supported by CyberChef but a rare choice for EVP_BytesToKey;
  skipped to keep the enum focused (md5/sha1/sha256/sha512 cover OpenSSL's common
  `-md` values). Easy to add later if requested.
- **Parsing a `Salted__…` file header** to auto-extract the salt — would need a
  file-input surface; the user can paste the 8 salt bytes as hex today.

## Verification (all surfaces)

- **Unit tests (8):** OpenSSL-verified vectors — MD5 no-salt (`5d41402a…`/`de37085f…`),
  MD5 + 8-byte salt (`577943ad…`/`f8fd4a50…`), SHA-256 no-salt (`5e884898…`/`3b029028…`),
  MD5("password") single-block, salt-encoding agreement, base64 roundtrip, iteration
  count, error paths. All pass.
- **Drift guard:** `schema_json_matches_authored_chat_schema` passes (no LLM-schema drift).
- **Chat block:** `wafer build` validates `target/block.wasm` (339.9 KiB).
- **CLI:** `gizza tool evp-derive password=password hash=sha256` → key/iv match OpenSSL;
  bad-hash error path returns a clean message + exit 1.
- **Page:** 3 Playwright tests pass (SHA-256 vector, MD5+salt vector, query-param deep link).

## Sources

- [EVP_BytesToKey — OpenSSL 3.4 docs](https://docs.openssl.org/3.4/man3/EVP_BytesToKey/)
- [EVP_BytesToKey(3) — Linux man page](https://linux.die.net/man/3/evp_bytestokey)
- [anothermh/evp_bytes_to_key (Ruby)](https://github.com/anothermh/evp_bytes_to_key)
