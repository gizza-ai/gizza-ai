## About this tool

PBKDF2 (Password-Based Key Derivation Function 2, defined in RFC 2898 and RFC 8018)
stretches a password into a fixed-length cryptographic key by applying an HMAC hash
many times over. This tool runs PBKDF2-HMAC entirely in your browser using compiled
Rust (WebAssembly) — your password and salt are never sent anywhere.

### Inputs

- **Password** — the passphrase to derive a key from.
- **Salt** — a value mixed in so the same password yields different keys. For real
  key derivation, use a unique, random salt per key. The salt can be plain text
  (utf8), or raw bytes given as **hex** or **base64**.
- **Iterations** — how many times the HMAC is applied. Higher means slower to compute
  and harder to brute-force. The default is 100,000; OWASP currently recommends about
  600,000 for PBKDF2-HMAC-SHA256.
- **Hash** — the underlying HMAC function: **SHA-256** (default), **SHA-512**, or
  **SHA-1** (legacy compatibility only).
- **Key length** — the number of output bytes (default 32 = 256 bits).
- **Output encoding** — **hex** (default) or **base64**.

The result is deterministic: identical inputs always produce the same derived key,
so you can reproduce a key on any device or platform that implements PBKDF2.

### Verify mode

Set **Mode** to `verify` and paste an existing key (hex or base64) into **Expected
key** to check whether your password and parameters reproduce it. The expected key's
byte length sets the derived length automatically, and the comparison runs locally.

### Common uses

- Reproduce a derived key that another system (OpenSSL, Python `hashlib.pbkdf2_hmac`,
  Node's `crypto.pbkdf2`, WebCrypto) produced, to confirm a match.
- Generate an encryption key from a passphrase for a personal project.
- Learn how iteration count and hash choice change the output and the time cost.

### Notes on security

PBKDF2 is widely supported and FIPS-approved, but it is not memory-hard. For new
password-storage designs, a memory-hard function such as Argon2id or scrypt resists
GPU/ASIC attacks better. Whatever you choose, always use a unique random salt and the
highest iteration count your latency budget allows.

### Test vectors

This tool matches the published PBKDF2 test vectors (RFC 6070 for HMAC-SHA1 and
RFC 7914 for HMAC-SHA256), so its output is interoperable with standard libraries.

## FAQ

<details>
<summary>My key doesn't match another library — what's different?</summary>

PBKDF2 output depends on **every** parameter: password, salt (and how the salt
bytes are decoded), iteration count, HMAC hash and key length. A mismatch is
almost always one of these — most often the salt encoding (this tool defaults
to treating the salt as UTF-8 text; set it to hex or base64 if the other side
used raw bytes) or a different iteration count.

</details>

<details>
<summary>How do I verify a password against an existing key?</summary>

Switch **Mode** to `verify` and paste the existing key (hex or base64,
auto-detected) into **Expected key**. The tool derives with your password and
parameters and compares — the expected key's byte length automatically sets
the derived length, so you don't have to enter it. Everything runs locally.

</details>

<details>
<summary>What limits are there on iterations and key length?</summary>

Iterations must be at least 1 (there's no upper cap — very high counts just
take longer, since the work runs in your browser), and the derived key length
is 1 to 1024 bytes. The default is 100,000 iterations and a 32-byte
(256-bit) key; OWASP currently suggests ~600,000 for PBKDF2-HMAC-SHA256.

</details>

<details>
<summary>Should I use PBKDF2 or Argon2 for new password storage?</summary>

PBKDF2 is widely supported and FIPS-approved, but it isn't memory-hard, so it's
weaker against GPU/ASIC cracking. For a brand-new design prefer a memory-hard
function like Argon2id or scrypt. Whatever you pick, always use a unique random
salt and the highest iteration count your latency budget allows.

</details>
