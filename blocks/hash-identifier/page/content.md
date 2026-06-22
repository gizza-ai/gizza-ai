## About this tool

The **Hash Identifier** inspects a hash string and tells you which algorithm or
password-hashing scheme most likely produced it. Recognition is purely
**structural** — the tool never tries to crack or reverse the hash, it only
classifies the format.

### How it works

- **Prefixed / structured formats** are matched by their unambiguous marker and
  reported with high confidence: bcrypt (`$2a$`/`$2b$`/`$2y$`), Argon2
  (`$argon2id$`, `$argon2i$`, `$argon2d$`), scrypt, sha256crypt (`$5$`),
  sha512crypt (`$6$`), md5crypt (`$1$`), PHPass / WordPress / phpBB
  (`$P$` / `$H$`), Drupal 7 (`$S$`), Apache apr1 (`$apr1$`), Cisco IOS type 8/9,
  PBKDF2 (Django/passlib), LDAP schemes (`{SHA}`, `{SSHA}`, `{SSHA512}`, …),
  MySQL 4.1+ (`*…`) and captured NetNTLM responses.
- **Bare hex digests** are matched by length, so a single width can map to a
  whole family of algorithms. A 32-character hex string, for example, is
  reported as **MD5, NTLM, MD4 or MD2** — all plausible candidates, ordered by
  how common they are.
- **Base64 digests** are classified by their approximate decoded byte length
  (e.g. ~20 bytes → SHA-1, ~32 bytes → SHA-256).

### Why a hash can have several answers

Many cryptographic hashes share the same output size, so a bare digest is
genuinely ambiguous — you cannot tell a 256-bit SHA-256 from a SHA3-256 or a
BLAKE2s-256 by looking at it alone. That is why the tool lists every match
instead of guessing one, and labels each with a confidence level.

### Privacy

Everything runs locally in your browser via WebAssembly. The hash you paste is
never uploaded to a server.
