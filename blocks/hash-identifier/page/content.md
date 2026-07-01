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

## FAQ

<details>
<summary>Why do I get several candidates for one hash?</summary>

Because many algorithms share an output size, a bare digest is genuinely
ambiguous — a 64-char hex string could be SHA-256, SHA3-256, or BLAKE2s-256.
The tool lists every structural match ordered by how common it is, and tags each
with a **high / medium / low** confidence: prefixed formats like `$2b$` (bcrypt)
or `$argon2id$` are high confidence, bare hex/base64 digests are low.

</details>

<details>
<summary>Does it tell me the hashcat mode to use?</summary>

Yes — when a candidate maps to a single Hashcat `-m` mode number (e.g. bcrypt
= 3200, NetNTLMv2 = 5600) it is included with the match, so you can go straight
from identification to a cracking or auditing command. Grouped ambiguous
candidates that span multiple modes don't carry a number.

</details>

<details>
<summary>Can this tool crack or reverse the hash?</summary>

No. Recognition is purely structural: it looks at prefixes, length, and character
set to classify the format. It never attempts to recover the original password or
plaintext, and since it runs entirely in your browser the hash is never sent
anywhere.

</details>

<details>
<summary>What happens if my string isn't a known hash format?</summary>

You get an explicit "no match" result (and an error for empty input) rather than
a bad guess. Common causes: extra whitespace, a truncated digest, or an encoded
wrapper (e.g. hex wrapped in base64) — try trimming or decoding one layer first.

</details>
