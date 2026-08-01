# key-derive — competitor analysis (2026-07-27)

**Tool:** `key-derive` — one unified KDF selector that derives a key of a chosen
length from a passphrase/seed via **PBKDF2, scrypt, Argon2id, or HKDF**, with the
per-algorithm parameters exposed. Runs locally (Rust→wasm); the secret never leaves
the device.

## Scan (top real KDF references/tools)

1. **Crypt::KeyDerivation (Perl / CryptX)** —
   <https://metacpan.org/pod/Crypt::KeyDerivation>. A single module exposing PBKDF1,
   PBKDF2, HKDF, bcrypt, scrypt and Argon2 behind one API — the closest analogue to a
   *unified selector*. Confirms the value of one surface that switches algorithm rather
   than four separate functions. Takes password, salt, output length, and
   algorithm-specific params. **Gap it validates:** a single `algorithm` selector +
   `length` is the expected shape.
2. **PyCryptodome — Protocol.KDF** —
   <https://pycryptodome.readthedocs.io/en/v3.23.0/src/protocol/kdf.html>. `PBKDF2`,
   `scrypt`, `HKDF`, `Argon2` all produce **raw key bytes of a caller-chosen `dkLen`**
   (Argon2 via `argon2` returning raw bytes, not a PHC string). This is the exact
   capability our existing `argon2-hash` block does NOT cover (it emits a PHC hash for
   password storage). **Gap it validates:** raw Argon2id key material of chosen length
   is a distinct, wanted output.
3. **pyca/cryptography KDFs** —
   <https://cryptography.io/en/latest/hazmat/primitives/key-derivation-functions/>.
   Documents each KDF's real knobs: PBKDF2 (hash + iterations), Scrypt (n/r/p),
   HKDF (hash + salt + `info` context), Argon2id (memory/iterations/lanes). Also states
   HKDF "is not suitable for deriving keys from a password" — it expands existing
   high-entropy key material. **Gaps it validates:** expose `info` for HKDF; expose
   n/r/p, memory/time/lanes; document that HKDF wants a seed/key, not a password.

## Decisions (in-model → built into this tool)

- **Single `algorithm` selector** — `pbkdf2 | scrypt | argon2 | hkdf`; each
  algorithm's own params are separate descriptor fields, described as
  "(<algo> only)" so an LLM/CLI user knows when they apply. (matches #1, #2)
- **Chosen output length** in bytes for every algorithm, including **raw Argon2id**
  key material via `Argon2::hash_password_into` (NOT a PHC string — that stays in the
  focused `argon2-hash` block). (matches #2 — the distinguishing capability)
- **Per-algorithm params:** PBKDF2 `iterations` + `hash`; scrypt `n`/`r`/`p`;
  Argon2 `memory_kib`/`time_cost`/`parallelism` + `argon2_variant`
  (argon2id/argon2i/argon2d); HKDF `hash` + `info`/`info_encoding`. (matches #3)
- **Input + salt encodings** utf8/hex/base64, output encoding hex/base64. Deterministic
  output for reproducibility across libraries. (matches #1–#3)
- **Runs locally, secret never uploaded** — the browser/CLI privacy angle the
  code-library competitors can't offer.

## Out of model (not built — noted)

- **bcrypt / PBKDF1** (from #1) — separate primitives; bcrypt has its own focused shape
  and PBKDF1 is deprecated. Not part of this selector.
- **PHC hash-string output + verify** for password storage — already covered by the
  focused `argon2-hash` (Argon2), `pbkdf2-derive`, `scrypt-derive` (verify modes).
  This tool is the raw-key-material derivation surface, kept single-purpose.

## Relationship to existing blocks (not a duplicate)

`pbkdf2-derive`, `scrypt-derive`, `hkdf-derive` are single-algorithm blocks with
derive/verify/extract modes; `argon2-hash` emits a **PHC hash** (password storage),
not raw chosen-length Argon2 bytes. `key-derive` is the **unified selector** and is the
**only** surface producing **raw Argon2id key material of a chosen length** — a distinct
faithful capability, not a union rehash.

## Sources

- [Crypt::KeyDerivation — metacpan.org](https://metacpan.org/pod/Crypt::KeyDerivation)
- [Key Derivation Functions — PyCryptodome](https://pycryptodome.readthedocs.io/en/v3.23.0/src/protocol/kdf.html)
- [Key derivation functions — pyca/cryptography](https://cryptography.io/en/latest/hazmat/primitives/key-derivation-functions/)
